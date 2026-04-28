(function_item name: (identifier) @symbol) @function
(struct_item name: (type_identifier) @symbol) @struct
(enum_item name: (type_identifier) @symbol) @enum
(trait_item name: (type_identifier) @symbol) @trait
(const_item name: (identifier) @symbol) @const
(static_item name: (identifier) @symbol) @static
(impl_item
  trait: _? @impl_trait
  type: (type_identifier) @symbol) @impl
