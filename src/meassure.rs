pub struct Measured<T> {
    pub data: Option<T>,
    pub measurements: Vec<(String, std::time::Duration)>,
}

impl<T> Measured<T> {
    pub fn new() -> Self {
        Self {
            data: None,
            measurements: Vec::new(),
        }
    }

    pub fn set(&mut self, data: T) {
        self.data = Some(data);
    }

    pub fn measure<Ret>(&mut self, name: impl ToString, func: impl FnOnce() -> Ret) -> Ret {
        let now = std::time::Instant::now();
        let ret = func();

        self.measurements.push((
            name.to_string(),
            std::time::Instant::now().duration_since(now),
        ));

        ret
    }

    pub fn data(&mut self) -> T {
        self.data.take().unwrap()
    }

    pub fn append<D>(mut self, other: Measured<D>) -> Measured<D> {
        self.measurements.extend(other.measurements);
        Measured {
            data: other.data,
            measurements: self.measurements,
        }
    }
}
