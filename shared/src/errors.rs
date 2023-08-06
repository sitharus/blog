pub struct RecoverableError<T>
where
    T: AsRef<str>,
{
    message: T,
}
