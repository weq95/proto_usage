#[derive(Clone)]
struct Duck;

impl Duck {
    fn swim(&self) -> String {
        String::from("Look, the duck is swimming")
    }
}

impl Bird for Duck {
    fn quack(&self) -> String {
        String::from("duck duck")
    }
}

#[derive(Clone)]
struct Swan;

impl Swan {
    fn fly(&self) -> String {
        String::from("Look, the duck.. oh sorry, the swan is flying")
    }
}

impl Bird for Swan {
    fn quack(&self) -> String {
        String::from("swan swan")
    }
}

trait Bird {
    fn quack(&self) -> String;
}

struct Manager<T: ?Sized> {
    items: Vec<Box<T>>,
}

impl<T: ?Sized + Bird> Manager<T> {
    fn new() -> Self {
        Self { items: Vec::new() }
    }

    fn add(&mut self, item: Box<T>) {
        self.items.push(item);
    }

    fn remove(&mut self, item: &T) {
        self.items.retain(|i| i.quack() != item.quack());
    }

    fn service(&mut self) {
        for i in self.items.iter() {
            println!("{}", i.quack());
        }
    }
}

fn main() {
    let mut items = Manager::new();
    let duck = Box::new(Duck);
    let bird = Box::new(Swan);

    items.add(duck.clone() as Box<dyn Bird>);
    items.add(bird.clone() as Box<dyn Bird>);
    items.service();

    println!("{}", duck.swim());
    println!("{}", bird.fly());
}
