use proc_macro2::TokenStream;
use quote::ToTokens;

pub fn err<T: ToTokens>(spanned: T, message: &str) -> TokenStream {
    syn::Error::new_spanned(spanned, message).to_compile_error()
}

pub const ERR_MISSING_PUB: &str = "klynt_command requires `pub`";
pub const ERR_MISSING_ASYNC: &str = "klynt_command requires `pub async fn`";
pub const ERR_DECLARED_STATE: &str =
    "klynt_command injects `state` automatically — remove this parameter";
pub const ERR_RESULT_RETURN: &str =
    "klynt_command wraps return type for you — declare bare `T` instead of `Result<T, ApiError>` or `CommandResult<T>`";
pub const ERR_MISSING_RETURN: &str = "klynt_command requires an explicit return type";
pub const ERR_NOT_FUNCTION: &str = "klynt_command can only be applied to functions";
