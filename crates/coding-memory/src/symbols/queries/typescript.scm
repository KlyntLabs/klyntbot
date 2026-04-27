(function_declaration name: (identifier) @symbol) @function
(class_declaration name: (type_identifier) @symbol) @class
(method_definition name: (property_identifier) @symbol) @method
(interface_declaration name: (type_identifier) @symbol) @interface
(type_alias_declaration name: (type_identifier) @symbol) @type
(lexical_declaration
  (variable_declarator
    name: (identifier) @symbol
    value: [(arrow_function) (function_expression)])) @const_arrow
(lexical_declaration
  (variable_declarator
    name: (identifier) @symbol)) @const
