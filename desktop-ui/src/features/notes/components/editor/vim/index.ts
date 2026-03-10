import type { Editor } from "@tiptap/react";
import { Extension } from "@tiptap/react";
import { createVimCursorPlugin } from "./cursor";
import { createVimSearchPlugin } from "./search";
import {
  createVimPlugin,
  type VimPluginInstance,
  type VimPluginOptions,
  vimPluginKey,
} from "./VimPlugin";
import type { VimMode, VimState } from "./VimState";

export type { VimState, VimMode };

export interface VimModeOptions {
  onStateChange?: (state: VimState) => void;
  onOpenCommandLine?: (prefix: string) => void;
  enabled?: () => boolean;
}

/**
 * Look up the VimPlugin instance from the editor's ProseMirror plugin registry.
 * Returns null if vim mode extension is not loaded.
 */
export function getVimPlugin(editor: Editor): VimPluginInstance | null {
  const plugin = vimPluginKey.get(editor.state);
  return plugin ? (plugin as unknown as VimPluginInstance) : null;
}

export const VimModeExtension = Extension.create<VimModeOptions>({
  name: "vimMode",

  addOptions() {
    return {
      onStateChange: undefined,
      onOpenCommandLine: undefined,
      enabled: undefined,
    };
  },

  addProseMirrorPlugins() {
    const getEnabled = this.options.enabled ?? (() => false);

    const pluginOpts: VimPluginOptions = {
      onStateChange: this.options.onStateChange ?? (() => {}),
      onOpenCommandLine: this.options.onOpenCommandLine ?? (() => {}),
      getEnabled,
    };

    const vimPlugin = createVimPlugin(pluginOpts);
    const getMode = (): VimMode => vimPlugin.getVimState().mode;
    const getPattern = (): string | null => vimPlugin.getVimState().searchPattern;

    return [
      vimPlugin,
      createVimCursorPlugin(getMode, getEnabled),
      createVimSearchPlugin(getPattern),
    ];
  },
});

export default VimModeExtension;
