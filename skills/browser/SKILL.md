# Browser Automation

Use the `browser` tool to navigate web pages and perform real-world tasks like booking tickets, shopping, and managing accounts.

## Core workflow

Always follow this sequence:
1. `navigate` to the target URL
2. `snapshot` to see all interactive elements as `@e1`, `@e2`, etc.
3. Interact using the `@e` references from the snapshot
4. `snapshot` again after navigation to refresh element references

## Action reference

| Action | When to use |
|---|---|
| `navigate` | Load a new URL |
| `snapshot` | Get current page elements (always do this before clicking) |
| `fill_form` | Fill multiple fields at once using label names |
| `login_flow` | Authenticate on a login page |
| `click` | Click a button or link by `@e` ref or label |
| `fill` | Fill a single input field |
| `type` | Type text character by character |
| `press` | Send a keyboard key (Enter, Tab, Escape) |
| `wait` | Wait for a page element or URL change |
| `get_text` | Extract text from an element |
| `screenshot` | Capture the current page state |
| `submit_and_confirm` | Click a submit/checkout button (always requires confirmation) |

## Write action confirmation

When you receive a `[CONFIRMATION_REQUIRED]` response from the browser tool:
1. Use `ask_user` to show the user what action is about to happen
2. Wait for their confirmation
3. If confirmed, call the same browser action again

Example:
```
browser: {"action": "click", "element": "@e5 Place Order"}
→ [CONFIRMATION_REQUIRED] About to click 'Place Order'. Use ask_user to confirm...

ask_user: "I'm about to click 'Place Order' on checkout.amazon.com. Shall I proceed?"
→ User: "Yes"

browser: {"action": "click", "element": "@e5"}  ← repeat the call
```

## Common task patterns

### Shopping
```
navigate → snapshot → fill_form (search) → snapshot → click (product) →
snapshot → click "Add to Cart" → snapshot → submit_and_confirm (checkout)
```

### Booking
```
navigate → snapshot → fill_form (dates/passengers) → snapshot →
click (search/available option) → snapshot → fill_form (passenger details) →
submit_and_confirm (book)
```

### Login
```
login_flow (url + username + password)
```

## Tips
- `@e` references expire after navigation — always snapshot again after a page change
- Use `fill_form` instead of individual `fill` calls for multi-field forms
- Use `screenshot` to verify state after complex interactions
- If an element is not found by label, try `snapshot` and inspect the raw output
