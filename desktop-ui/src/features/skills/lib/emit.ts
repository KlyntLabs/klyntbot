export function emitSkillsUpdated() {
  window.dispatchEvent(new CustomEvent("skills:updated"));
}
