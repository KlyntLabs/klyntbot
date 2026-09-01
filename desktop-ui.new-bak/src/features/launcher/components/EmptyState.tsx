interface Props {
  query: string;
}

export function EmptyState({ query }: Props) {
  return (
    <div className="lc-empty" role="status">
      <div className="lc-empty-icon">🔍</div>
      <p className="lc-empty-title">No results for "{query}"</p>
      <ul className="lc-empty-hints">
        <li>
          <kbd>f/</kbd> Files
        </li>
        <li>
          <kbd>g/</kbd> Grep
        </li>
        <li>
          <kbd>h/</kbd> History
        </li>
        <li>
          <kbd>@</kbd> Contacts
        </li>
        <li>
          <kbd>{">"}</kbd> Commands
        </li>
        <li>
          <kbd>?</kbd> Ask AI
        </li>
      </ul>
    </div>
  );
}
