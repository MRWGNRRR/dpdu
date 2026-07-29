use proc_macro::TokenStream;

mod worker_rpc;

#[proc_macro]
pub fn declare_worker_rpc(input: TokenStream) -> TokenStream {
    worker_rpc::declare_worker_rpc(input)
}
