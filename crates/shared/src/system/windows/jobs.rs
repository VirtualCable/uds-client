// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
//    this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
//    this list of conditions and the following disclaimer in the documentation
//    and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
//    may be used to endorse or promote products derived from this software
//    without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

// Authors: Adolfo Gómez, dkmaster at dkmon dot com
use super::{event::Event, safe::SafeHandle};
use std::collections::HashSet;
use std::slice;
use windows::Win32::{
    Foundation::{HANDLE, HWND},
    System::JobObjects::{
        CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JobObjectBasicAccountingInformation, JobObjectExtendedLimitInformation,
        QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
    },
    UI::WindowsAndMessaging::IsWindowVisible,
};

use super::utils::check_if_any_visible_window;
use crate::log;

#[allow(dead_code)]
pub fn create_job_object() -> SafeHandle {
    let job: SafeHandle = unsafe { CreateJobObjectW(None, None).unwrap() }
        .try_into()
        .unwrap();

    let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

    unsafe {
        SetInformationJobObject(
            job.get(),
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const _,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
        .unwrap();
    }

    job
}

#[allow(dead_code)]
pub fn terminate_job(job: SafeHandle) -> anyhow::Result<()> {
    unsafe {
        TerminateJobObject(job.get(), 1).map_err(|e| {
            anyhow::anyhow!("Failed to terminate job object: {:?}, error: {:?}", job, e)
        })?;
    }
    Ok(())
}

#[allow(dead_code)]
pub fn get_job_accounting_info(job: HANDLE) -> JOBOBJECT_BASIC_ACCOUNTING_INFORMATION {
    let mut basic_info: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION =
        JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
    unsafe {
        QueryInformationJobObject(
            Some(job),
            JobObjectBasicAccountingInformation,
            &mut basic_info as *mut _ as *mut _,
            std::mem::size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
            None,
        )
        .unwrap();
    }
    basic_info
}

pub fn get_job_active_process_ids(job: HANDLE) -> Vec<usize> {
    use windows::Win32::System::JobObjects::{
        JOBOBJECT_BASIC_PROCESS_ID_LIST, JobObjectBasicProcessIdList, QueryInformationJobObject,
    };

    let mut buffer = [0u8; 4096];

    let ptr = buffer.as_mut_ptr();
    let list_ptr = ptr as *mut JOBOBJECT_BASIC_PROCESS_ID_LIST;

    unsafe {
        QueryInformationJobObject(
            Some(job),
            JobObjectBasicProcessIdList,
            list_ptr as *mut _,
            buffer.len() as u32,
            None,
        )
        .unwrap();

        let list = &*list_ptr;
        let count = list
            .NumberOfAssignedProcesses
            .min(list.NumberOfProcessIdsInList) as usize;

        // Obtain the slice of process IDs
        let pids_ptr = list.ProcessIdList.as_ptr();

        let pid_slice = slice::from_raw_parts(pids_ptr, count);

        //pid_slice.iter().map(|&pid| pid).filter(|&pid| pid != 0).collect()

        let hash: HashSet<usize> = pid_slice.iter().cloned().filter(|pid| *pid != 0).collect();
        hash.into_iter().collect()
    }
}

pub fn wait_for_job(job: SafeHandle, stop_event: Event) -> anyhow::Result<()> {
    let mut last_visible_window: Option<isize> = None;
    // Store the time, so the first 10 seconds we don't check for the window visibility
    let start: std::time::Instant = std::time::Instant::now();
    loop {
        // If last_visible_window is Some, check if it still is visible
        let still_visible: bool = if let Some(hwnd) = last_visible_window {
            if unsafe { IsWindowVisible(HWND(hwnd as _)) }.as_bool() {
                log::debug!(
                    "Last visible window {:?} is still visible, continuing wait loop.",
                    hwnd
                );
                true
            } else {
                log::debug!(
                    "Last visible window {:?} is no longer visible, checking job state.",
                    hwnd
                );
                false
            }
        } else {
            log::debug!("No last visible window to check, continuing wait loop.");
            false
        };

        if !still_visible {
            // If the last visible window is not visible, we need to check the job state
            log::debug!("Checking job state for job: {:?}", job);
            let pids = get_job_active_process_ids(job.get());
            if pids.is_empty() {
                log::debug!(
                    "No active processes found in job {}, exiting wait loop.",
                    job
                );
                break;
            }

            if start.elapsed().as_secs() > 10 {
                // Encapsulate to avoid Send trait issues
                let visible_window = check_if_any_visible_window(&pids);
                log::debug!(
                    "Job {} has {} active processes, has visible windows: {:?}",
                    job,
                    pids.len(),
                    visible_window
                );

                if visible_window.is_none() {
                    log::debug!(
                        "No visible windows found for job {}, exiting wait loop.",
                        job
                    );
                    break;
                }
                if let Some(hwnd) = visible_window {
                    last_visible_window = Some(hwnd.0 as isize);
                }
            }
        }
        // Read the event_handler to see if we should stop waiting
        if stop_event.wait_timeout(std::time::Duration::from_millis(100)) {
            log::debug!("Event signaled, exiting wait loop for job: {:?}", job);
            // Ensure job is terminated
            if let Err(e) = terminate_job(job.clone()) {
                log::error!("Failed to terminate job {}: {:?}", job, e);
            }
            break;
        }
    }
    // Return Ok if we exited the loop without errors
    log::debug!("Exited wait loop for job: {:?}", job);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_terminate_job() {
        let job = create_job_object();
        assert!(job.is_valid());

        // In fact, here, the job is empty

        assert!(terminate_job(job).is_ok());
    }

    #[test]
    fn test_wait_for_job() {}
}
