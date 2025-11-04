use super::{event::Event, safe::SafeHandle};
use std::collections::HashSet;
use std::slice;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::JobObjects::{
    CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectBasicAccountingInformation,
    JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
    TerminateJobObject,
};

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
    // Store the time, so the first 10 seconds we don't check for the window visibility
    loop {
        // If the last visible window is not visible, we need to check the job state
        let pids = get_job_active_process_ids(job.get());
        if pids.is_empty() {
            log::debug!(
                "No active processes found in job {}, exiting wait loop.",
                job
            );
            break;
        }
        // Read the event_handler to see if we should stop waiting
        if stop_event.wait_timeout(std::time::Duration::from_millis(300)) {
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
