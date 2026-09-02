export function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <span className="text-ui-sm font-medium text-fg-secondary uppercase tracking-wider">
      {children}
    </span>
  );
}
