use std::panic::AssertUnwindSafe;

use sc_executor::{
	error::{Error, Result},
	with_externalities_safe, Externalities, HeapAllocStrategy, RuntimeVersion, RuntimeVersionOf,
};

pub use sc_executor::{NativeExecutionDispatch, NativeVersion, WasmExecutor};
use sp_core::traits::{CallContext, CodeExecutor, RuntimeCode};
use sp_version::GetNativeVersion;
use sp_wasm_interface::ExtendedHostFunctions;

/// Supports selecting execution backend and manages runtime cache.
#[derive(Debug, Clone)]
pub struct WasmStrategy {
	/// The heap allocation strategy for onchain Wasm calls.
	default_onchain_heap_alloc_strategy: HeapAllocStrategy,
	/// The heap allocation strategy for offchain Wasm calls.
	default_offchain_heap_alloc_strategy: HeapAllocStrategy,
	/// Ignore onchain heap pages value.
	ignore_onchain_heap_pages: bool,
}

/// A generic `CodeExecutor` implementation that uses a delegate to determine wasm code equivalence
/// and dispatch to native code when possible, falling back on `WasmExecutor` when not.
pub struct NativeElseWasmExecutor<D: NativeExecutionDispatch> {
	native_version: NativeVersion,
	wasm: Option<
		WasmExecutor<ExtendedHostFunctions<sp_io::SubstrateHostFunctions, D::ExtendHostFunctions>>,
	>,
	wasm_strategy: Option<WasmStrategy>,
	use_native: bool,
}

impl<D: NativeExecutionDispatch> NativeElseWasmExecutor<D> {
	/// Create a new instance using the given [`WasmExecutor`].
	pub fn new_with_wasm_executor(
		executor: WasmExecutor<
			ExtendedHostFunctions<sp_io::SubstrateHostFunctions, D::ExtendHostFunctions>,
		>,
		strategy: WasmStrategy,
	) -> Self {
		Self {
			native_version: D::native_version(),
			wasm: Some(executor),
			wasm_strategy: Some(strategy),
			use_native: true,
		}
	}

	/// Create a new instance using the given native runtime.
	pub fn new_with_native_executor() -> Self {
		Self {
			native_version: D::native_version(),
			wasm: None,
			wasm_strategy: None,
			use_native: true,
		}
	}

	/// Disable to use native runtime when possible just behave like `WasmExecutor`.
	///
	/// Default to enabled.
	pub fn disable_use_native(&mut self) {
		self.use_native = false;
	}
}

impl<D: NativeExecutionDispatch> RuntimeVersionOf for NativeElseWasmExecutor<D> {
	fn runtime_version(
		&self,
		ext: &mut dyn Externalities,
		runtime_code: &RuntimeCode,
	) -> Result<RuntimeVersion> {
		if let Some(ref wasm) = &self.wasm {
			wasm.runtime_version(ext, runtime_code)
		} else {
			Ok(self.native_version.runtime_version.clone())
		}
	}
}

impl<D: NativeExecutionDispatch> GetNativeVersion for NativeElseWasmExecutor<D> {
	fn native_version(&self) -> &NativeVersion {
		&self.native_version
	}
}

impl<D: NativeExecutionDispatch + 'static> CodeExecutor for NativeElseWasmExecutor<D> {
	type Error = Error;
	fn call(
		&self,
		ext: &mut dyn Externalities,
		runtime_code: &RuntimeCode,
		method: &str,
		data: &[u8],
		_use_native: bool,
		context: CallContext,
	) -> (std::result::Result<Vec<u8>, Self::Error>, bool) {
		let use_native = self.use_native;

		tracing::trace!(
			target: "executor",
			function = %method,
			"Executing function",
		);

		if let Some(ref wasm) = &self.wasm {
			let heap_alloc_strategy = if let Some(ref strategy) = &self.wasm_strategy {
				let on_chain_heap_alloc_strategy = if strategy.ignore_onchain_heap_pages {
					strategy.default_onchain_heap_alloc_strategy
				} else {
					runtime_code
						.heap_pages
						.map(|h| HeapAllocStrategy::Static {
							extra_pages: h as _,
						})
						.unwrap_or_else(|| strategy.default_onchain_heap_alloc_strategy)
				};

				match context {
					CallContext::Offchain => strategy.default_offchain_heap_alloc_strategy,
					CallContext::Onchain => on_chain_heap_alloc_strategy,
				}
			} else {
				return (
					Err(Error::Other("Wasm Strategy not set".into())),
					use_native,
				);
			};

			let mut used_native = false;

			let result = wasm.with_instance(
				runtime_code,
				ext,
				heap_alloc_strategy,
				|_, mut instance, onchain_version, mut ext| {
					let onchain_version =
						onchain_version.ok_or_else(|| Error::ApiError("Unknown version".into()))?;

					let can_call_with =
						onchain_version.can_call_with(&self.native_version.runtime_version);

					if use_native && can_call_with {
						tracing::trace!(
							target: "executor",
							native = %self.native_version.runtime_version,
							chain = %onchain_version,
							"Request for native execution succeeded",
						);

						used_native = true;
						Ok(
							with_externalities_safe(&mut **ext, move || D::dispatch(method, data))?
								.ok_or_else(|| Error::MethodNotFound(method.to_owned())),
						)
					} else {
						if !can_call_with {
							tracing::trace!(
								target: "executor",
								native = %self.native_version.runtime_version,
								chain = %onchain_version,
								"Request for native execution failed",
							);
						}

						with_externalities_safe(&mut **ext, move || {
							instance.call_export(method, data)
						})
					}
				},
			);
			(result, used_native)
		} else {
			let used_native = true;
			let result = {
				let mut ext = AssertUnwindSafe(ext);

				with_externalities_safe(&mut **ext, move || D::dispatch(method, data))
					.unwrap()
					.ok_or_else(|| Error::MethodNotFound(method.to_owned()))
			};

			(result, used_native)
		}
	}
}

impl<D: NativeExecutionDispatch> Clone for NativeElseWasmExecutor<D> {
	fn clone(&self) -> Self {
		NativeElseWasmExecutor {
			native_version: D::native_version(),
			wasm: self.wasm.clone(),
			wasm_strategy: self.wasm_strategy.clone(),
			use_native: self.use_native,
		}
	}
}

impl<D: NativeExecutionDispatch> sp_core::traits::ReadRuntimeVersion for NativeElseWasmExecutor<D> {
	fn read_runtime_version(
		&self,
		wasm_code: &[u8],
		ext: &mut dyn Externalities,
	) -> std::result::Result<Vec<u8>, String> {
		if let Some(ref wasm) = &self.wasm {
			wasm.read_runtime_version(wasm_code, ext)
		} else {
			unreachable!()
		}
	}
}
