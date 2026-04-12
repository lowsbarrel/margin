import { Extension } from "@tiptap/core";
import { PluginKey } from "@tiptap/pm/state";
import Suggestion, { type SuggestionOptions } from "@tiptap/suggestion";

export interface SlashMenuItem {
  title: string;
  description: string;
  icon: string;
  searchTerms: string[];
  command: (props: { editor: any; range: any }) => void;
}

export const slashMenuPluginKey = new PluginKey("slash-command");

const SlashCommand = Extension.create({
  name: "slash-command",

  addOptions() {
    return {
      suggestion: {
        char: "/",
        command: ({ editor, range, props }: any) => {
          props.command({ editor, range });
        },
        allow: ({ state, range }: any) => {
          const $from = state.doc.resolve(range.from);
          if ($from.parent.type.name === "codeBlock") {
            return false;
          }
          return true;
        },
      } as Partial<SuggestionOptions>,
    };
  },

  addProseMirrorPlugins() {
    return [
      Suggestion({
        pluginKey: slashMenuPluginKey,
        ...this.options.suggestion,
        editor: this.editor,
      }),
    ];
  },
});

export default SlashCommand;
