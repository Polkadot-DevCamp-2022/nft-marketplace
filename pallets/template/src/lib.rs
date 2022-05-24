#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/v3/runtime/frame>
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[derive(Clone, Encode, Decode, MaxEncodedLen, TypeInfo)]
	pub struct Order {
		pub token_id: u64,
		pub sell_price: u64,
	}

	// The pallet's runtime storage items.
	// https://docs.substrate.io/v3/runtime/storage
	#[pallet::storage]
	#[pallet::getter(fn get_next_token_id)]
	// Learn more about declaring storage items:
	// https://docs.substrate.io/v3/runtime/storage#declaring-storage-items
	pub type NextTokenId<T> = StorageValue<_, u64>;

	#[pallet::storage]
	#[pallet::getter(fn get_nft_details)]
	pub type TokenIdToOwner<T: Config> = StorageMap<_,Blake2_128Concat, u64, (T::AccountId, u64), OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn get_number_of_sell_orders)]
	pub type NumberOfSellOrders<T> = StorageValue<_, u128>;

	#[pallet::storage]
	#[pallet::getter(fn get_sell_order)]
	pub type SellOrders<T> = StorageMap<_, Blake2_128Concat, u128, Order, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn is_onsale)]
	pub type IsTokenOnSale<T> = StorageMap<_, Blake2_128Concat, u64, u128, OptionQuery>;

	#[pallet::storage] 
	#[pallet::getter(fn get_number_of_nfts_owned)] 
	pub type OwnerToNumberOfNFTs<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, u64, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn get_token_ids_of_owned_nfts)]
	pub type OwnerToTokenIds<T: Config> = StorageDoubleMap<_, Blake2_128Concat, T::AccountId, Blake2_128Concat, u64, u64, OptionQuery>;

	// Pallets use events to inform users when important changes are made.
	// https://docs.substrate.io/v3/runtime/events-and-errors
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event documentation should end with an array that provides descriptive names for event
		/// parameters. [tokenId, owner]
		NFTMinted(u64, T::AccountId),
		SellOrderCreated(u64, u64),
		CancelledOrder(u64),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// Error names should be descriptive.
		NoneValue,
		/// Errors should have helpful documentation associated with them.
		StorageOverflow,
		/// Token already minted
		TokenIdAlreadyMinted,
		/// Nfts doesn't exist for given tokenid
		InValidTokenId,
		/// 
		NotTokenOwner,
		TokenAlreadyOnSale,
		TokenNotOnSale,
		SellOrderNotFound,
		NoSellOrdersFound,

	}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Mints an NFT
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn mint(_origin: OriginFor<T>) -> DispatchResult {
			// Check that the extrinsic was signed and get the signer.
			let owner = ensure_signed(_origin)?;

			// Gets token_id and updates NextTokenId
			let token_id: u64 = match <NextTokenId<T>>::get() {
				None => {
					<NextTokenId<T>>::put(1);
					0u64
				},
				Some(val) => {
					let new_val = val.checked_add(1).ok_or(Error::<T>::StorageOverflow)?;
					<NextTokenId<T>>::put(new_val);
					val
				},
			};

			// Gets index of the current nfts for the owner
			let number_of_nfts: u64 = match <OwnerToNumberOfNFTs<T>>::get(&owner) {
				None => {
					<OwnerToNumberOfNFTs<T>>::insert(&owner, 1);
					0
				},
				Some(val) => {
					let new_val = val.checked_add(1).ok_or(Error::<T>::StorageOverflow)?;
					<OwnerToNumberOfNFTs<T>>::insert(&owner, new_val);
					val
				}
			};

			// Ensures the tokenid is not already minted
			ensure!(!TokenIdToOwner::<T>::contains_key(&token_id), Error::<T>::TokenIdAlreadyMinted);

			// Adds record of tokenids owner
			TokenIdToOwner::<T>::insert(&token_id, (&owner, &number_of_nfts));

			// Adds tokenid to owners list of owned tokenids
			OwnerToTokenIds::<T>::insert(&owner, &number_of_nfts, &token_id);

			// Emit an event.
			Self::deposit_event(Event::NFTMinted(token_id, owner));
			// Return a successful DispatchResultWithPostInfo
			Ok(())
		}

		/// Sell an nft
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1,1))]
		pub fn sell(_origin: OriginFor<T>, _token_id: u64, _price: u64) -> DispatchResult {

			// Check that the extrinsic was signed and get the signer.
			let who = ensure_signed(_origin)?;

			// Check tokenid validity
			ensure!(TokenIdToOwner::<T>::contains_key(&_token_id), Error::<T>::InValidTokenId);
			
			// Get Owner of tokenid
			let (token_owner, _) = TokenIdToOwner::<T>::get(&_token_id).expect("All tokenIds have a owner");

			// Check if who is the owner of the token
			ensure!(who == token_owner, Error::<T>::NotTokenOwner);

			ensure!(!IsTokenOnSale::<T>::contains_key(&_token_id), Error::<T>::TokenAlreadyOnSale);

			let new_order = Order {
				token_id: _token_id,
				sell_price: _price,
			};

			let number_of_sell_orders = match NumberOfSellOrders::<T>::get() {
				None => {
					NumberOfSellOrders::<T>::put(1);
					0
				},
				Some(val) => {
					let new_val = val.checked_add(1).ok_or(Error::<T>::StorageOverflow)?;
					NumberOfSellOrders::<T>::put(new_val);
					val
				}
			};

			SellOrders::<T>::insert(&number_of_sell_orders, &new_order);

			IsTokenOnSale::<T>::insert(&_token_id, &number_of_sell_orders);

			Self::deposit_event(Event::SellOrderCreated(_token_id, _price));

			Ok(())
		}

		// Cancel a sell order
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1,1))]
		pub fn cancel_order(_origin: OriginFor<T>, _token_id: u64) -> DispatchResult {
			// Check that the extrinsic was signed and get the signer.
			let who = ensure_signed(_origin)?;

			// Check tokenid validity
			ensure!(TokenIdToOwner::<T>::contains_key(&_token_id), Error::<T>::InValidTokenId);
			
			// Get Owner of tokenid
			let (token_owner, _) = TokenIdToOwner::<T>::get(&_token_id).expect("All tokenIds have a owner");

			// Check if who is the owner of the token
			ensure!(who == token_owner, Error::<T>::NotTokenOwner);

			// Check if token is on sale 
			ensure!(IsTokenOnSale::<T>::contains_key(&_token_id), Error::<T>::TokenNotOnSale);

			// Get the index of the order in SellOrders
			let index_in_sell_orders = IsTokenOnSale::<T>::get(&_token_id).expect("All tokens on sale have a index");

			// Get the index of the last order in SellOrders
			let last_index_in_sell_orders = match NumberOfSellOrders::<T>::get() {
				None => {
					Err(Error::<T>::NoSellOrdersFound)?;
					0
				},
				Some(val) => {
					if val == 0 {
						Err(Error::<T>::NoSellOrdersFound)?;
						val
					} else {
						val - 1
					}
				},
			}; 


			if index_in_sell_orders != last_index_in_sell_orders {
				// Check if SellOrders have a order in last index
				ensure!(SellOrders::<T>::contains_key(&last_index_in_sell_orders), Error::<T>::SellOrderNotFound);

				// Order at the last index
				let order_at_last_index = SellOrders::<T>::get(&last_index_in_sell_orders).expect("All tokens on sale have an order");

				let token_id_of_last_order = order_at_last_index.token_id;
				
				// Inser last order at index of deleted order
				SellOrders::<T>::insert(&index_in_sell_orders, &order_at_last_index);

				IsTokenOnSale::<T>::insert(&token_id_of_last_order, &index_in_sell_orders);
			}

			// Remove the token id from isTokenOnSale
			IsTokenOnSale::<T>::remove(&_token_id);

			SellOrders::<T>::remove(&last_index_in_sell_orders);

			NumberOfSellOrders::<T>::put(&last_index_in_sell_orders);

			Self::deposit_event(Event::CancelledOrder(_token_id));

			Ok(())
		}
	}
}
