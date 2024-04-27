#[macro_use]
extern crate serde;
use candid::{Decode, Encode};
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct Rental {
    id: u64,
    customer_name: String,
    ps_model: String,
    rental_start_time: u64,
    rental_end_time: Option<u64>,
}

// a trait that must be implemented for a struct that is stored in a stable struct
impl Storable for Rental {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

// another trait that must be implemented for a struct that is stored in a stable struct
impl BoundedStorable for Rental {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    static ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create a counter")
    );

    static STORAGE: RefCell<StableBTreeMap<u64, Rental, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1)))
    ));
}

#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct RentalPayload {
    customer_name: String,
    ps_model: String,
}

#[ic_cdk::query]
fn get_rental(id: u64) -> Result<Rental, Error> {
    match _get_rental(&id) {
        Some(rental) => Ok(rental),
        None => Err(Error::NotFound {
            msg: format!("a rental with id={} not found", id),
        }),
    }
}

#[ic_cdk::update]
fn add_rental(rental: RentalPayload) -> Option<Rental> {
    let id = ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("cannot increment id counter");
    let rental = Rental {
        id,
        customer_name: rental.customer_name,
        ps_model: rental.ps_model,
        rental_start_time: time(),
        rental_end_time: None,
    };
    do_insert(&rental);
    Some(rental)
}

#[ic_cdk::update]
fn update_rental(id: u64, payload: RentalPayload) -> Result<Rental, Error> {
    match STORAGE.with(|service| service.borrow().get(&id)) {
        Some(mut rental) => {
            rental.customer_name = payload.customer_name;
            rental.ps_model = payload.ps_model;
            rental.rental_end_time = Some(time());
            do_insert(&rental);
            Ok(rental)
        }
        None => Err(Error::NotFound {
            msg: format!("couldn't update a rental with id={}. rental not found", id),
        }),
    }
}

// helper method to perform insert.
fn do_insert(rental: &Rental) {
    STORAGE.with(|service| service.borrow_mut().insert(rental.id, rental.clone()));
}

#[ic_cdk::update]
fn delete_rental(id: u64) -> Result<Rental, Error> {
    match STORAGE.with(|service| service.borrow_mut().remove(&id)) {
        Some(rental) => Ok(rental),
        None => Err(Error::NotFound {
            msg: format!("couldn't delete a rental with id={}. rental not found.", id),
        }),
    }
}

#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
}

// a helper method to get a rental by id. used in get_rental/update_rental
fn _get_rental(id: &u64) -> Option<Rental> {
    STORAGE.with(|service| service.borrow().get(id))
}

// need this to generate candid
ic_cdk::export_candid!();