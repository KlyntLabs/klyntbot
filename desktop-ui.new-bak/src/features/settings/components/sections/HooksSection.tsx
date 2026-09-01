export function HooksSection() {
  // TODO: Wire to unified hooks_list command once backend exposes it.
  // Previously invoked coding_hooks_list (removed in unify-to-assistant).
  return (
    <div className="hooks-section">
      <p>
        No <code>~/.klyntbot/hooks.toml</code> found.
      </p>
      <p>Hooks are user-managed; create the file to enable.</p>
    </div>
  );
}
