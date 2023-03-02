use std::{
    sync::{Mutex, Condvar, Arc},
    env,
    thread,
    fmt::{self, Display},
};

struct BoundedBuffer<const BOUND: usize> {
    array: [isize; BOUND],
    n_items: usize,
}
impl<const BOUND: usize> BoundedBuffer<BOUND> {

    fn new() -> Self { BoundedBuffer { array: [0; BOUND], n_items: 0 } }

    fn empty(&self) -> bool { self.n_items == 0     }
    fn full (&self) -> bool { self.n_items == BOUND }

    fn push(&mut self, item: isize) {
        assert!(!self.full());
        self.array[self.n_items] = item;
        self.n_items += 1;
    }
    fn pop(&mut self) -> isize {
        assert!(!self.empty());
        self.n_items -= 1;
        self.array[self.n_items]
    }
}
impl<const BOUND: usize> Default for BoundedBuffer<BOUND> {
    fn default() -> Self { Self::new() }
}
impl<const BOUND: usize> Display for BoundedBuffer<BOUND> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {

        write!(f, "[")?;

        if self.n_items > 0 {
            write!(f, "{}", self.array[0])?;
            for i in 1..self.n_items { write!(f, ", {}", self.array[i])?; };
        };

        write!(f, "]")
    }
}

#[derive(Default)]
struct SyncedBoundedBuffer<const BOUND: usize> {
    buffer: Mutex<BoundedBuffer<BOUND>>,
    not_empty: Condvar,
    not_full: Condvar,
}

fn producer_routine<const BOUND: usize>(sbbuf: Arc<SyncedBoundedBuffer<BOUND>>, item: isize) {
    loop {
        // acquire the mutex so we can (at least) check if the buffer is full
        let mut bbuf = sbbuf.buffer.lock().unwrap();

        /* If the buffer is full, release the mutex until it isn't full.
        `wait` blocks until `not_full` is signalled; the `while` instead of `if` is because it is possible for
        the buffer to be full when `wait` returns, as follows:
            1. the buffer becomes not full and `not_full` is signalled, waking all producers
            2. another producer thread runs before this one, and fills the buffer
            3. then this thread runs.
        */
        while bbuf.full() { bbuf = sbbuf.not_full.wait(bbuf).unwrap(); }

        // add an item to the buffer
        bbuf.push(item);
        // display the buffer state
        println!("{}", bbuf);

        // since we just pushed an item, the buffer is definitely not empty.
        // We use `notify_all` instead of `notify_one` because there may be space for multiple items, which
        // may be filled by multiple threads.
        sbbuf.not_empty.notify_all();
        // we're done; now the MutexGuard goes out of scope, unlocking the Mutex
    }
}

// see the producer routine for comments
fn consumer_routine<const BOUND: usize>(sbbuf: Arc<SyncedBoundedBuffer<BOUND>>) {
    loop {
        let mut bbuf = sbbuf.buffer.lock().unwrap();
        while bbuf.empty() { bbuf = sbbuf.not_empty.wait(bbuf).unwrap(); }

        bbuf.pop();
        println!("{}", bbuf);

        sbbuf.not_full.notify_all();
    }
}

fn main() {
    const BUF_SIZE: usize = 30; // arbitary choice

    let mut args = env::args();
    args.next(); // ignore program name
    let n_producers = args.next().expect("missing argument: n_producers").parse::<usize>().unwrap();
    let n_consumers = args.next().expect("missing argument: n_consumers").parse::<usize>().unwrap();

    let mut producers = Vec::with_capacity(n_producers);
    let mut consumers = Vec::with_capacity(n_consumers);

    let bounded_buffer = Arc::from(SyncedBoundedBuffer::<BUF_SIZE>::default());

    // spawn the threads
    for i in 0..n_producers {
        let buf = bounded_buffer.clone();
        producers.push( thread::spawn(move || producer_routine(buf, i as isize)) );
    }
    for _ in 0..n_consumers {
        let buf = bounded_buffer.clone();
        consumers.push( thread::spawn(move || consumer_routine(buf)) );
    }

    // wait for all threads to complete (which will never happen since they're infinite loops)
    for thread in producers { thread.join().unwrap(); };
    for thread in consumers { thread.join().unwrap(); };
}
