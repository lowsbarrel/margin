import { Extension } from "@tiptap/core";
import { PluginKey } from "@tiptap/pm/state";
import Suggestion, { type SuggestionOptions } from "@tiptap/suggestion";

export interface MentionItem {
  title: string;
  path: string;
}

export const mentionPluginKey = new PluginKey("mention-command");

const MentionCommand = Extension.create({
  name: "mention-command",

  addOptions() {
    return {
      suggestion: {
        char: "@",
        command: ({ editor, range, props }: any) => {
          editor
            .chain()
            .focus()
            .deleteRange(range)
            .insertWikiLink(props.title)
            .run();
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
        pluginKey: mentionPluginKey,
        ...this.options.suggestion,
        editor: this.editor,
      }),
    ];
  },
});

export default MentionCommand;
