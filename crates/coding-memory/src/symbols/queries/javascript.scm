(function_declaration name: (identifier) @symbol) @function
(class_declaration name: (identifier) @symbol) @class
(method_definition name: (property_identifier) @symbol) @method
(lexical_declaration
  (variable_declarator
    name: (identifier) @symbol
    value: [(arrow_function) (function_expression)])) @const_arrow
(lexical_declaration
  (variable_declarator
    name: (identifier) @symbol)) @const
