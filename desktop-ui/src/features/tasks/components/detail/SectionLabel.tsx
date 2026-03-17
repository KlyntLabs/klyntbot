export function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <span className="text-xs font-medium text-muted uppercase tracking-wider">
      {children}
    </span>
  );
}
