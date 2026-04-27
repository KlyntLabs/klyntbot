export function LoadingState() {
  return (
    <div className="lc-loading" role="status" aria-busy="true">
      {Array.from({ length: 6 }).map((_, i) => (
        <div key={i} className="lc-loading-row">
          <div className="lc-loading-icon" />
          <div className="lc-loading-text">
            <div className="lc-loading-line" />
            <div className="lc-loading-line short" />
          </div>
        </div>
      ))}
    </div>
  );
}
