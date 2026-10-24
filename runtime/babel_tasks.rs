// Small task primitive for Babel: generated functions may execute concurrently.
// Tasks are native threads with explicitly awaitable structured Data results.
#[derive(Clone)]
struct BabelTask {
    shared: std::sync::Arc<(
        std::sync::Mutex<Option<BabelData>>,
        std::sync::Condvar
    )>,
}
impl BabelText for BabelTask {
    fn babel_text(&self) -> String {
        if babel_task_ready(self.clone()) { "<task:ready>".into() }
        else { "<task:pending>".into() }
    }
}
fn babel_task_spawn<F: FnOnce(BabelData) -> BabelData + Send + 'static>(
    f: F, input: BabelData
) -> BabelTask {
    let shared = std::sync::Arc::new((
        std::sync::Mutex::new(None),
        std::sync::Condvar::new()
    ));
    let output = BabelTask { shared: shared.clone() };
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(input)));
        let value = result.unwrap_or_else(|_| babel_result(false, BabelData::Null, "task panicked"));
        let (lock, cv) = &*shared;
        *lock.lock().expect("Task lock poisoned") = Some(value);
        cv.notify_all();
    });
    output
}
fn babel_task_ready(task: BabelTask) -> bool {
    let (lock, _) = &*task.shared;
    lock.lock().expect("Task lock poisoned").is_some()
}
fn babel_task_wait(task: BabelTask) -> BabelData {
    let (lock, cv) = &*task.shared;
    let mut state = lock.lock().expect("Task lock poisoned");
    while state.is_none() {
        state = cv.wait(state).expect("Task lock poisoned");
    }
    state.as_ref().unwrap().clone()
}
fn babel_task_poll(task: BabelTask) -> BabelData {
    let ready = babel_task_ready(task.clone());
    let value = if ready { babel_task_wait(task) } else { BabelData::Null };
    babel_result(ready, value, "")
}
