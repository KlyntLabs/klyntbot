type ThreadLoadingProps = {
  nested?: boolean;
};

export function ThreadLoading({ nested }: ThreadLoadingProps) {
  return (
    <div
      className={nested ? "flex flex-col gap-2 pl-4" : "flex flex-col gap-2"}
      role="status"
      aria-label="Loading agents"
    >
      <span className="h-2 w-[78%] rounded-full bg-gradient-to-r from-white/[0.04] via-white/[0.18] to-white/[0.04] bg-[length:200%_100%] animate-[shimmer_1.4s_ease-in-out_infinite]" />
      <span className="h-2 w-[62%] rounded-full bg-gradient-to-r from-white/[0.04] via-white/[0.18] to-white/[0.04] bg-[length:200%_100%] animate-[shimmer_1.4s_ease-in-out_infinite]" />
      <span className="h-2 w-[44%] rounded-full bg-gradient-to-r from-white/[0.04] via-white/[0.18] to-white/[0.04] bg-[length:200%_100%] animate-[shimmer_1.4s_ease-in-out_infinite]" />
    </div>
  );
}
