#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {

	use frame_support::{
		pallet_prelude::*,
		transactional,
		traits::{Currency, tokens::ExistenceRequirement},
	};
	use frame_system::pallet_prelude::*;
	use scale_info::TypeInfo;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type Currency: Currency<Self::AccountId>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	type BalanceOf<T> =
    	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	#[derive(Clone, Encode, Decode, MaxEncodedLen, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	#[codec(mel_bound())]
	pub struct Order<T: Config> {
		pub token_id: u64,
		pub sell_price: BalanceOf<T>,
	}

	type TokenID = u64;

	#[pallet::storage]
	#[pallet::getter(fn get_next_token_id)]
	pub type NextTokenId<T> = StorageValue<_, TokenID>;

	#[pallet::storage]
	#[pallet::getter(fn get_nft_details)]
	pub type TokenIdToOwner<T: Config> = StorageMap<_,Blake2_128Concat, TokenID, (T::AccountId, u64), OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn get_number_of_sell_orders)]
	pub type NumberOfSellOrders<T> = StorageValue<_, u128>;

	#[pallet::storage]
	#[pallet::getter(fn get_sell_order)]
	pub type SellOrders<T> = StorageMap<_, Blake2_128Concat, u128, Order<T>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn is_onsale)]
	pub type IsTokenOnSale<T> = StorageMap<_, Blake2_128Concat, TokenID, u128, OptionQuery>;

	#[pallet::storage] 
	#[pallet::getter(fn get_number_of_nfts_owned)] 
	pub type OwnerToNumberOfNFTs<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, u64, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn get_token_ids_of_owned_nfts)]
	pub type OwnerToTokenIds<T: Config> = StorageDoubleMap<_, Blake2_128Concat, T::AccountId, Blake2_128Concat, u64, TokenID, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// [TokenID, Minter] 
		NFTMinted(TokenID, T::AccountId),
		/// [TokenID, Price]
		SellOrderCreated(TokenID, BalanceOf<T>),
		/// [TokenID]
		CancelledOrder(TokenID),
		/// [Buyer, Seller, Price]
		NFTSold(T::AccountId, T::AccountId, BalanceOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Number of items exceeded than supported
		StorageOverflow,
		/// Given tokenID is already minted
		TokenIdAlreadyMinted,
		/// NFT doesn't exist for the given tokenID
		InvalidTokenID,
		/// You are not the owner of this token
		NotTokenOwner,
		/// Cannot add the same token twice
		TokenAlreadyOnSale,
		/// Given token is not available for purchase
		TokenNotOnSale,
		/// Invalid Sell order Id
		SellOrderNotFound,
		/// Empty marketplace
		NoSellOrdersFound,
		/// Insufficient fund to purchase NFT
		NotEnoughBalance,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Mints a NFT
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(2,4))]
		pub fn mint(_origin: OriginFor<T>) -> DispatchResult {
			// Check that the extrinsic was signed and get the signer.
			let owner = ensure_signed(_origin)?;

			// Gets token_id and updates NextTokenId
			let token_id: TokenID = <NextTokenId<T>>::get().unwrap_or(0);
			<NextTokenId<T>>::put(token_id.checked_add(1).ok_or(Error::<T>::StorageOverflow)?);

			// Gets index of the current nfts for the owner
			let number_of_nfts = <OwnerToNumberOfNFTs<T>>::get(&owner).unwrap_or(0);
			<OwnerToNumberOfNFTs<T>>::insert(
				&owner,
				number_of_nfts + 1
			);

			// Adds record of tokenIds owner
			TokenIdToOwner::<T>::insert(&token_id, (&owner, &number_of_nfts));

			// Adds tokenId to owners list of owned tokenIds
			OwnerToTokenIds::<T>::insert(&owner, &number_of_nfts, &token_id);

			Self::deposit_event(Event::NFTMinted(token_id, owner));
			Ok(())
		}

		/// Sell NFT
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(3,3))]
		pub fn sell(_origin: OriginFor<T>, _token_id: TokenID, _price: BalanceOf<T>) -> DispatchResult {

			// Check that the extrinsic was signed and get the signer.
			let who = ensure_signed(_origin)?;
			
			// Get Owner of tokenid
			let (token_owner, _) = match TokenIdToOwner::<T>::get(&_token_id) {
				Some(x) => x,
				None => Err(<Error<T>>::InvalidTokenID)?
			};

			// Check if who is the owner of the token
			ensure!(who == token_owner, Error::<T>::NotTokenOwner);

			ensure!(!IsTokenOnSale::<T>::contains_key(&_token_id), Error::<T>::TokenAlreadyOnSale);

			let new_order = Order {
				token_id: _token_id,
				sell_price: _price,
			};

			let number_of_sell_orders = NumberOfSellOrders::<T>::get().unwrap_or(0);
			NumberOfSellOrders::<T>::put(
				number_of_sell_orders.
					checked_add(1).
					ok_or(Error::<T>::StorageOverflow)?
			);

			SellOrders::<T>::insert(&number_of_sell_orders, &new_order);
			IsTokenOnSale::<T>::insert(&_token_id, &number_of_sell_orders);

			Self::deposit_event(Event::SellOrderCreated(_token_id, _price));

			Ok(())
		}

		// Cancel a sell order
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(5,6))]
		pub fn cancel_order(_origin: OriginFor<T>, _token_id: u64) -> DispatchResult {
			// Check that the extrinsic was signed and get the signer.
			let who = ensure_signed(_origin)?;

			// Get Owner of tokenid
			let (token_owner, _) = match TokenIdToOwner::<T>::get(&_token_id) {
				Some(x) => x,
				None => Err(<Error<T>>::InvalidTokenID)?
			};

			// Check if who is the owner of the token
			ensure!(who == token_owner, Error::<T>::NotTokenOwner);

			// Get the index of the order in SellOrders
			let index_in_sell_orders = match IsTokenOnSale::<T>::get(&_token_id) {
				Some(id) => id,
				None => Err(<Error<T>>::TokenNotOnSale)?
			};

			Self::destroy_sell_order(index_in_sell_orders)?;
			Self::deposit_event(Event::CancelledOrder(_token_id));

			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(9,11))]
		#[transactional]
		pub fn buy(_origin: OriginFor<T>, _token_id: TokenID) -> DispatchResult {
			let buyer = ensure_signed(_origin)?;

			let sell_id = match Self::is_onsale(_token_id) {
				Some(id) => id,
				None => Err(<Error<T>>::TokenNotOnSale)?
			};

			let (seller, idx) = Self::get_nft_details(_token_id).unwrap();
			let sell_price = Self::get_sell_order(sell_id).unwrap().sell_price;
			
			// Transfer balance
			ensure!(T::Currency::free_balance(&buyer) >= sell_price, <Error<T>>::NotEnoughBalance);
			T::Currency::transfer(&buyer, &seller, sell_price, ExistenceRequirement::KeepAlive)?;

			// Delete sell order
			Self::destroy_sell_order(sell_id)?;

			// Remove seller as the owner of the NFT
			let seller_nft_count = Self::get_number_of_nfts_owned(&seller).unwrap();

			<OwnerToNumberOfNFTs<T>>::insert(
				&seller,
				seller_nft_count - 1
			);

			if idx != (seller_nft_count - 1) {
				let last_nft_id = Self::get_token_ids_of_owned_nfts(&seller, seller_nft_count - 1).unwrap();
				OwnerToTokenIds::<T>::insert(&seller, idx, last_nft_id);
			}
			OwnerToTokenIds::<T>::remove(&seller, seller_nft_count-1);

			// Make buyer the owner of the NFT
			let buyer_nft_count = Self::get_number_of_nfts_owned(&buyer).unwrap_or(0);

			TokenIdToOwner::<T>::insert(_token_id, (&buyer, &buyer_nft_count));

			<OwnerToNumberOfNFTs<T>>::insert(
				&buyer,
				buyer_nft_count.checked_add(1).ok_or(Error::<T>::StorageOverflow)?
			);

			OwnerToTokenIds::<T>::insert(&buyer, &buyer_nft_count, &_token_id);

			Self::deposit_event(Event::NFTSold(buyer, seller, sell_price));
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		
		fn destroy_sell_order(index_in_sell_orders: u128) -> Result<(), Error<T>> {

			let token_id: TokenID = SellOrders::<T>::get(index_in_sell_orders).unwrap().token_id;

			// Get the index of the last order in SellOrders
			let last_index_in_sell_orders = NumberOfSellOrders::<T>::get().unwrap() - 1;

			if index_in_sell_orders != last_index_in_sell_orders {

				// Order at the last index
				let order_at_last_index = match SellOrders::<T>::get(&last_index_in_sell_orders) {
					Some(order) => order,
					None => Err(<Error<T>>::SellOrderNotFound)?
				};

				let token_id_of_last_order = order_at_last_index.token_id;
				
				// Insert last order at index of deleted order
				SellOrders::<T>::insert(&index_in_sell_orders, &order_at_last_index);
				IsTokenOnSale::<T>::insert(&token_id_of_last_order, &index_in_sell_orders);
			}

			// Remove the token id from isTokenOnSale
			IsTokenOnSale::<T>::remove(&token_id);
			SellOrders::<T>::remove(&last_index_in_sell_orders);
			NumberOfSellOrders::<T>::put(&last_index_in_sell_orders);

			Ok(())
		}
	}
}
