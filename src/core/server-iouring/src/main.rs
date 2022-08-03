
#[cfg(target_os = "linux")]
pub fn main() {
    use server_iouring::Worker;
    use std::sync::mpsc::channel;
    use server_iouring::Listener;


    let (l_tx, w_rx) = channel();
    let (w_tx, l_rx) = channel();

    let mut threads = Vec::new();

    let listener = Listener::new(l_tx, l_rx).expect("failed to init listener");
    threads.push(std::thread::spawn(|| { 
        listener.run()
    }));

    let worker = Worker::new(w_tx, w_rx).expect("failed to init worker");
    threads.push(std::thread::spawn(|| { 
        worker.run()
    }));

    for thread in threads {
        thread.join();
    }
}

#[cfg(not(target_os = "linux"))]
pub fn main() {
    println!("io_uring is only supported on linux");
}