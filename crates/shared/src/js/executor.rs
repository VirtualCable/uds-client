use anyhow::Result;
use boa_engine::context::time::JsInstant;
use boa_engine::job::{GenericJob, TimeoutJob};
use boa_engine::{
    Context, JsResult, JsValue, Script, Source,
    context::ContextBuilder,
    job::{Job, JobExecutor, NativeAsyncJob, PromiseJob},
};
use futures::{StreamExt, stream::FuturesUnordered};
use std::collections::BTreeMap;
use std::ops::DerefMut;
use std::{cell::RefCell, collections::VecDeque, rc::Rc};
use tokio::task;

use crate::log;

/// An event queue using tokio to drive futures to completion.
struct Queue {
    async_jobs: RefCell<VecDeque<NativeAsyncJob>>,
    promise_jobs: RefCell<VecDeque<PromiseJob>>,
    timeout_jobs: RefCell<BTreeMap<JsInstant, TimeoutJob>>,
    generic_jobs: RefCell<VecDeque<GenericJob>>,
}

impl Queue {
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
                eprintln!("Uncaught {e}");
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
            eprintln!("Uncaught {err}");
        }

        let jobs = std::mem::take(&mut *self.promise_jobs.borrow_mut());
        for job in jobs {
            if let Err(e) = job.call(context) {
                eprintln!("Uncaught {e}");
            }
        }
        context.clear_kept_objects();
    }
}

impl JobExecutor for Queue {
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
    fn run_jobs(self: Rc<Self>, context: &mut Context) -> JsResult<()> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();

        task::LocalSet::default().block_on(&runtime, self.run_jobs_async(&RefCell::new(context)))
    }

    // ...the async flavor won't, which allows concurrent execution with external async tasks.
    async fn run_jobs_async(self: Rc<Self>, context: &RefCell<&mut Context>) -> JsResult<()> {
        let mut jobs = FuturesUnordered::new();

        loop {
            // Insertamos todos los async jobs pendientes
            for job in std::mem::take(&mut *self.async_jobs.borrow_mut()) {
                jobs.push(job.call(context));
            }

            // Si no queda nada pendiente, salimos
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

pub fn create_context() -> Result<Context> {
    let queue = Rc::new(Queue::new());
    let ctx = ContextBuilder::new()
        .job_executor(queue.clone())
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to create JS context: {}", e))?;

    Ok(ctx)
}

pub async fn exec_script(ctx: &mut Context, script: &str) -> Result<JsValue> {
    let script = Script::parse(Source::from_bytes(script.as_bytes()), None, ctx).unwrap();

    // `Script::evaluate_async` will yield to the executor from time to time, Unlike `Context::run`
    // or `Script::evaluate` which block the current thread until the execution finishes.
    log::debug!("Evaluating script...");
    let script_result = script.evaluate_async(ctx).await.unwrap();

    // Run the jobs asynchronously, which avoids blocking the main thread.
    log::debug!("Running jobs...");
    let queue: Rc<Queue> = ctx.downcast_job_executor().unwrap();

    queue
        .run_jobs_async(&RefCell::new(ctx))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to run pending jobs after script execution: {}", e))?;

    Ok(script_result)
}
