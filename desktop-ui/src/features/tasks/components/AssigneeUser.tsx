import { CircleUserRound } from "lucide-react";
import { Avatar, AvatarFallback, AvatarImage } from "./ui/avatar";

interface User {
  id: string;
  name: string;
  avatarUrl: string;
}

interface AssigneeUserProps {
  user: User | null;
}

export function AssigneeUser({ user }: AssigneeUserProps) {
  if (user) {
    return (
      <Avatar className="size-6 shrink-0">
        <AvatarImage src={user.avatarUrl} alt={user.name} />
        <AvatarFallback>{user.name[0] ?? "?"}</AvatarFallback>
      </Avatar>
    );
  }
  return (
    <div className="size-6 flex items-center justify-center">
      <CircleUserRound className="size-5 text-fg-secondary" />
    </div>
  );
}
