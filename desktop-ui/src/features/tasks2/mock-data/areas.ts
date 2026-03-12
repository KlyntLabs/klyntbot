// mock-data/areas.ts
export interface MockArea {
  id: string;
  name: string;
  projectIds: string[];
}

export const areas: MockArea[] = [
  { id: "area-work", name: "Work", projectIds: ["1", "2", "3", "4", "5"] },
  { id: "area-personal", name: "Personal", projectIds: ["6", "7"] },
  { id: "area-side", name: "Side Projects", projectIds: ["8", "9", "10"] },
];

export function getAreaById(id: string): MockArea | undefined {
  return areas.find((a) => a.id === id);
}

export function getAreaForProject(projectId: string): MockArea | undefined {
  return areas.find((a) => a.projectIds.includes(projectId));
}
