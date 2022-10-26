use std::{
    cell::UnsafeCell,
    io::{ErrorKind, Read, Write},
    sync::{Arc, Mutex, MutexGuard},
};

struct Inner<T> {
    locked: Mutex<Arc<UnsafeCell<T>>>,
}

/// Utility class allowing reading and writing from the same stream
pub struct CloneableStream<T> {
    reader: Arc<Inner<T>>,
    writer: Arc<Inner<T>>,
}

impl<T> Clone for CloneableStream<T> {
    fn clone(&self) -> Self {
        Self {
            reader: self.reader.clone(),
            writer: self.writer.clone(),
        }
    }
}

impl<T> CloneableStream<T> {
    pub fn new(data: T) -> CloneableStream<T> {
        let data = Arc::new(UnsafeCell::new(data));
        let reader = Inner {
            locked: Mutex::new(data.clone()),
        };

        let writer = Inner {
            locked: Mutex::new(data),
        };

        CloneableStream {
            reader: Arc::new(reader),
            writer: Arc::new(writer),
        }
    }

    fn get_ref<'a>(obj: &'a mut Arc<Inner<T>>) -> std::io::Result<(MutexGuard<'a, Arc<UnsafeCell<T>>>, &mut T)> {
        let lock = obj.locked.lock().map_err(|_| ErrorKind::Other)?;
        let t = lock.get();
        unsafe { Ok((lock, &mut *t)) }
    }
}

impl<T> Write for CloneableStream<T>
where
    T: Write,
{
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let (_lock, obj) = CloneableStream::get_ref(&mut self.writer)?;
        obj.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let (_lock, obj) = CloneableStream::get_ref(&mut self.writer)?;
        obj.flush()
    }
}

impl<T> Read for CloneableStream<T>
where
    T: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let (_lock, obj) = CloneableStream::get_ref(&mut self.reader)?;
        obj.read(buf)
    }
}

unsafe impl<T: Send> Send for CloneableStream<T> {}
unsafe impl<T: Sync> Sync for CloneableStream<T> {}
