// Small task primitive for Ain: generated functions may execute concurrently.
// Tasks are native threads with explicitly awaitable structured Data results.
#[derive(Clone)]
struct AinTask {
    shared: std::sync::Arc<(
        std::sync::Mutex<Option<AinData>>,
        std::sync::Condvar
    )>,
}
impl AinText for AinTask {
    fn ain_text(&self) -> String {
        if ain_task_ready(self.clone()) { "<task:ready>".into() }
        else { "<task:pending>".into() }
    }
}
fn ain_task_spawn<F: FnOnce(AinData) -> AinData + Send + 'static>(
    f: F, input: AinData
) -> AinTask {
    let shared = std::sync::Arc::new((
        std::sync::Mutex::new(None),
        std::sync::Condvar::new()
    ));
    let output = AinTask { shared: shared.clone() };
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(input)));
        let value = result.unwrap_or_else(|_| ain_result(false, AinData::Null, "task panicked"));
        let (lock, cv) = &*shared;
        *lock.lock().expect("Task lock poisoned") = Some(value);
        cv.notify_all();
    });
    output
}
fn ain_task_ready(task: AinTask) -> bool {
    let (lock, _) = &*task.shared;
    lock.lock().expect("Task lock poisoned").is_some()
}
fn ain_task_wait(task: AinTask) -> AinData {
    let (lock, cv) = &*task.shared;
    let mut state = lock.lock().expect("Task lock poisoned");
    while state.is_none() {
        state = cv.wait(state).expect("Task lock poisoned");
    }
    state.as_ref().unwrap().clone()
}
fn ain_task_poll(task: AinTask) -> AinData {
    let ready = ain_task_ready(task.clone());
    let value = if ready { ain_task_wait(task) } else { AinData::Null };
    ain_result(ready, value, "")
}
