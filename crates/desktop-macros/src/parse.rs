use syn::{FnArg, ItemFn, Pat, PatIdent, ReturnType, Type};

pub struct ParsedCommand {
    pub fn_item: ItemFn,
}

impl ParsedCommand {
    pub fn declared_state_param(&self) -> Option<&FnArg> {
        self.fn_item.sig.inputs.iter().find(|arg| {
            if let FnArg::Typed(pat_type) = arg
                && let Pat::Ident(PatIdent { ident, .. }) = &*pat_type.pat
            {
                return ident == "state";
            }
            false
        })
    }

    pub fn return_type(&self) -> Option<&Type> {
        if let ReturnType::Type(_, ty) = &self.fn_item.sig.output {
            Some(ty)
        } else {
            None
        }
    }

    pub fn return_type_is_result(&self) -> bool {
        // Detects `Result<T, _>` or `CommandResult<T>` literally.
        let Some(ty) = self.return_type() else {
            return false;
        };
        let s = quote::quote!(#ty).to_string();
        s.starts_with("Result <")
            || s.starts_with("CommandResult <")
            || s.contains(":: CommandResult <")
            || s.contains(":: Result <")
    }
}
