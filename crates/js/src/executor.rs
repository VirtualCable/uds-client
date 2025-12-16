// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.U.
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
use anyhow::Result;

use boa_engine::{
    Context, JsError, JsResult, JsValue, Module, Script, Source,
    builtins::promise::PromiseState,
    context::{ContextBuilder, time::JsInstant},
    job::{GenericJob, Job, JobExecutor, NativeAsyncJob, PromiseJob, TimeoutJob},
    module::MapModuleLoader,
    object::builtins::JsPromise,
};
use futures::{StreamExt, stream::FuturesUnordered};
use std::collections::BTreeMap;
use std::ops::DerefMut;
use std::{cell::RefCell, collections::VecDeque, rc::Rc};
use tokio::task;

use crate::log;

// Based on Boa example:
// https://github.com/boa-dev/boa/blob/main/examples/src/bin/tokio_event_loop.rs

/// An event queue using tokio to drive futures to completion.
struct JobQueue {
    async_jobs: RefCell<VecDeque<NativeAsyncJob>>,
    promise_jobs: RefCell<VecDeque<PromiseJob>>,
    timeout_jobs: RefCell<BTreeMap<JsInstant, TimeoutJob>>,
    generic_jobs: RefCell<VecDeque<GenericJob>>,
}

impl JobQueue {
    fn new() -> Self {
        Self {
            async_jobs: RefCell::default(),
            promise_jobs: RefCell::default(),
            timeout_jobs: RefCell::default(),
            generic_jobs: RefCell::default(),
        }
    }

    fn drain_timeout_jobs(&self, context: &mut Context) {
        let now = context.clock().now();

        let mut timeouts_borrow = self.timeout_jobs.borrow_mut();
        let mut jobs_to_keep = timeouts_borrow.split_off(&now);
        jobs_to_keep.retain(|_, job| !job.is_cancelled());
        let jobs_to_run = std::mem::replace(timeouts_borrow.deref_mut(), jobs_to_keep);
        drop(timeouts_borrow);

        for job in jobs_to_run.into_values() {
            if let Err(e) = job.call(context) {
                log::error!("Uncaught {e}");
            }
        }
    }

    fn drain_jobs(&self, context: &mut Context) {
        // Run the timeout jobs first.
        self.drain_timeout_jobs(context);

        let job = self.generic_jobs.borrow_mut().pop_front();
        if let Some(generic) = job
            && let Err(err) = generic.call(context)
        {
            log::error!("Uncaught {err}");
        }

        let jobs = std::mem::take(&mut *self.promise_jobs.borrow_mut());
        for job in jobs {
            if let Err(e) = job.call(context) {
                log::error!("Uncaught {e}");
            }
        }
        context.clear_kept_objects();
    }
}

impl JobExecutor for JobQueue {
    fn enqueue_job(self: Rc<Self>, job: Job, context: &mut Context) {
        match job {
            Job::PromiseJob(job) => self.promise_jobs.borrow_mut().push_back(job),
            Job::AsyncJob(job) => self.async_jobs.borrow_mut().push_back(job),
            Job::TimeoutJob(t) => {
                let now = context.clock().now();
                self.timeout_jobs.borrow_mut().insert(now + t.timeout(), t);
            }
            Job::GenericJob(g) => self.generic_jobs.borrow_mut().push_back(g),
            _ => panic!("unsupported job type"),
        }
    }

    // While the sync flavor of `run_jobs` will block the current thread until all the jobs have finished...
    fn run_jobs(self: Rc<Self>, _context: &mut Context) -> JsResult<()> {
        // Note: Not used, raise directly an error if called.
        Err(boa_engine::error::JsNativeError::error()
            .with_message("Synchronous job execution is not supported in this executor")
            .into())
    }

    // ...the async flavor won't, which allows concurrent execution with external async tasks.
    async fn run_jobs_async(self: Rc<Self>, context: &RefCell<&mut Context>) -> JsResult<()> {
        let mut jobs = FuturesUnordered::new();

        loop {
            // Insert all pending async jobs into the futures set
            for job in std::mem::take(&mut *self.async_jobs.borrow_mut()) {
                jobs.push(job.call(context));
            }

            // If there are no jobs left, we are done
            if jobs.is_empty()
                && self.promise_jobs.borrow().is_empty()
                && self.timeout_jobs.borrow().is_empty()
                && self.generic_jobs.borrow().is_empty()
            {
                return Ok(());
            }

            // Poll an async job
            if let Some(res) = jobs.next().await
                && let Err(err) = res
            {
                log::error!("Async job error: {}", err);
            }

            // Drain the other job types
            self.drain_jobs(&mut context.borrow_mut());

            // Yield to the Tokio runtime
            task::yield_now().await;
        }
    }
}

pub fn create_context(loader: Option<Rc<MapModuleLoader>>) -> Result<Context> {
    let queue = Rc::new(JobQueue::new());

    let mut ctx = ContextBuilder::new().job_executor(queue.clone());
    if let Some(loader) = loader {
        ctx = ctx.module_loader(loader);
    }

    let ctx = ctx
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to create JS context: {}", e))?;

    Ok(ctx)
}

pub async fn exec_script(ctx: &mut Context, script: &str) -> Result<()> {
    let module = Module::parse(Source::from_bytes(script.as_bytes()), None, ctx)
        .map_err(|e| anyhow::anyhow!("Failed to parse module: {}", e))?;

    // Load the module (returns a Promise)
    let promise = module.load(ctx);

    // Run jobs to process the load
    let queue = ctx
        .downcast_job_executor::<JobQueue>()
        .ok_or_else(|| anyhow::anyhow!("No job executor found"))?;

    queue
        .clone()
        .run_jobs_async(&RefCell::new(ctx))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to run pending jobs after module load: {}", e))?;

    // Check if load succeeded
    match promise.state() {
        PromiseState::Fulfilled(_) => {}
        PromiseState::Rejected(err) => {
            return Err(anyhow::anyhow!(
                "Module load failed: {}",
                JsError::from_opaque(err)
            ));
        }
        PromiseState::Pending => {
            return Err(anyhow::anyhow!("Module didn't finish loading"));
        }
    }

    // Link the module
    module
        .link(ctx)
        .map_err(|e| anyhow::anyhow!("Failed to link module: {}", e))?;

    // Evaluate the module (returns another Promise)
    let promise = module.evaluate(ctx);

    // Run jobs to process the evaluation
    queue
        .run_jobs_async(&RefCell::new(ctx))
        .await
        .map_err(|e| {
            anyhow::anyhow!("Failed to run pending jobs after module evaluation: {}", e)
        })?;

    // Get the result
    match promise.state() {
        PromiseState::Fulfilled(_value) => Ok(()), // On module, value is always undefined
        PromiseState::Rejected(err) => {
            let mut error: String = err.display().to_string();
            for frame in ctx.stack_trace() {
                let fnc_name = frame.position().function_name.clone().to_std_string_lossy();
                let line_info = match frame.position().position {
                    Some(pos) => format!(
                        "line: {}, column: {}",
                        pos.line_number(),
                        pos.column_number()
                    ),
                    None => "unknown line".to_string(),
                };
                log::error!("  at {} ({})", fnc_name, line_info);
                error += &format!("\n  at {} ({})", fnc_name, line_info);
            }

            Err(anyhow::anyhow!(
                // TODO: Add stack trace information?
                "Module evaluation failed: {}",
                error
            ))
        }
        PromiseState::Pending => Err(anyhow::anyhow!("Module didn't finish evaluating")),
    }
}

#[allow(dead_code)]
// Currently not used, but may will be useful later (probably :))
/// Note: On script mode, "await" is not allowed at top-level
pub async fn exec_script_with_result(ctx: &mut Context, script: &str) -> Result<JsValue> {
    let script = Script::parse(Source::from_bytes(script.as_bytes()), None, ctx)
        .map_err(|e| anyhow::anyhow!("Failed to parse script: {}", e))?;

    // `Script::evaluate_async` will yield to the executor from time to time, Unlike `Context::run`
    // or `Script::evaluate` which block the current thread until the execution finishes.
    log::debug!("Evaluating script...");
    let script_result = script
        .evaluate_async(ctx)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to execute script: {}", e))?;

    // Run the jobs asynchronously, which avoids blocking the main thread.
    log::debug!("Running jobs...");
    let queue = ctx
        .downcast_job_executor::<JobQueue>()
        .ok_or_else(|| anyhow::anyhow!("No job executor found in context"))?;

    queue
        .run_jobs_async(&RefCell::new(ctx))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to run pending jobs after script execution: {}", e))?;

    if let Some(obj) = script_result.as_object()
        && let Ok(promise) = JsPromise::from_object(obj.clone())
    {
        match promise.state() {
            PromiseState::Fulfilled(value) => {
                log::debug!("Promise fulfilled with value: {:?}", value);
                return Ok(value);
            }
            PromiseState::Rejected(err) => {
                log::error!("Promise was rejected with error: {:?}", err);
                return Err(anyhow::anyhow!(
                    "Promise was rejected with error: {:?}",
                    err
                ));
            }
            PromiseState::Pending => {
                log::warn!("Promise is still pending after job execution");
                return Err(anyhow::anyhow!(
                    "Promise is still pending after job execution"
                ));
            }
        }
    }

    Ok(script_result)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test tha we can execute and get result from a simple script
    #[tokio::test]
    async fn test_exec_script_with_result() -> Result<()> {
        let mut ctx = create_context(None)?;

        let script = r#"
            function add(a, b) {
                return a + b;
            }
            add(2, 3)
        "#;

        let result = exec_script_with_result(&mut ctx, script)
            .await
            .map_err(|e| anyhow::anyhow!("JavaScript execution error: {}", e))?;

        assert_eq!(result, JsValue::from(5));

        Ok(())
    }

    // Test that we can execute and get result from a script that returns an exception
    #[tokio::test]
    async fn test_exec_script_with_exception() -> Result<()> {
        let mut ctx = create_context(None)?;

        let script = r#"
            throw new Error("Test error");
        "#;

        let result = exec_script_with_result(&mut ctx, script).await;

        assert!(result.is_err());
        // Get the error message
        let err_msg = format!("{}", result.err().unwrap());
        assert!(err_msg.contains("Test error"));

        Ok(())
    }
}
