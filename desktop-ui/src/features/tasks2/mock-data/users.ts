export interface User {
  id: string;
  name: string;
  avatarUrl: string;
}

export const users: User[] = [
  { id: "1", name: "Alice Chen", avatarUrl: "" },
  { id: "2", name: "Bob Smith", avatarUrl: "" },
  { id: "3", name: "Carol Davis", avatarUrl: "" },
  { id: "4", name: "Dan Wilson", avatarUrl: "" },
];
