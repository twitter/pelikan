#[cfg(target_os = "linux")]
pub fn main() {
    use server_iouring::ListenerBuilder;
    use server_iouring::WorkerBuilder;

    let storage = entrystore::Noop::new();
    let parser = protocol_ping::RequestParser::new();

    let listener = ListenerBuilder::new(parser.clone()).expect("failed to init listener");

    let listener_waker = listener.waker();

    let worker = WorkerBuilder::new(parser, storage).expect("failed to init worker");

    let worker_waker = worker.waker();

    let (l_queue, w_queue) = server_iouring::queues(worker_waker, listener_waker);

    // let (l_tx, w_rx) = channel();
    // let (w_tx, l_rx) = channel();

    let mut threads = Vec::new();



    let listener = listener.build(l_queue).expect("failed to build listener");
    threads.push(std::thread::spawn(|| listener.run()));

    let worker = worker.build(w_queue).expect("failed to build worker");
    threads.push(std::thread::spawn(|| worker.run()));

    for thread in threads {
        thread.join();
    }
}

#[cfg(not(target_os = "linux"))]
pub fn main() {
    println!("io_uring is only supported on linux");
}
