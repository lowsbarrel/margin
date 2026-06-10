import { PluginKey } from "@tiptap/pm/state";
import { createSuggestionExtension } from "$lib/editor/suggestion-extension";

export interface MentionItem {
  title: string;
  path: string;
}

export const mentionPluginKey = new PluginKey("mention-command");

const MentionCommand = createSuggestionExtension<MentionItem>({
  name: "mention-command",
  char: "@",
  pluginKey: mentionPluginKey,
  command: ({ editor, range, props }) => {
    editor
      .chain()
      .focus()
      .deleteRange(range)
      .insertWikiLink(props.title)
      .run();
  },
});

export default MentionCommand;
