import type { Editor, Range } from "@tiptap/core";
import { PluginKey } from "@tiptap/pm/state";
import { createSuggestionExtension } from "$lib/editor/suggestion-extension";

export interface SlashMenuItem {
  title: string;
  description: string;
  icon: string;
  searchTerms: string[];
  command: (props: { editor: Editor; range: Range }) => void;
}

export const slashMenuPluginKey = new PluginKey("slash-command");

const SlashCommand = createSuggestionExtension<SlashMenuItem>({
  name: "slash-command",
  char: "/",
  pluginKey: slashMenuPluginKey,
  command: ({ editor, range, props }) => {
    props.command({ editor, range });
  },
});

export default SlashCommand;
