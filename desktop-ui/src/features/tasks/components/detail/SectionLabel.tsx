export function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <span className="text-xs font-medium text-[hsl(var(--muted-foreground))] uppercase tracking-wider">
      {children}
    </span>
  );
}
