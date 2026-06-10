import { Extension, type Editor, type Range } from "@tiptap/core";
import { PluginKey, type EditorState } from "@tiptap/pm/state";
import Suggestion, { type SuggestionOptions } from "@tiptap/suggestion";

/**
 * True when the suggestion range is not inside a code block. Used by every
 * suggestion extension's `allow` check so trigger characters (`/`, `@`, `:`)
 * stay inert while typing code.
 */
export function notInCodeBlock(state: EditorState, range: Range): boolean {
  const $from = state.doc.resolve(range.from);
  return $from.parent.type.name !== "codeBlock";
}

/**
 * Arguments passed to a suggestion `command` callback once an item is picked.
 */
export interface SuggestionCommandProps<TItem> {
  editor: Editor;
  range: Range;
  props: TItem;
}

export interface CreateSuggestionExtensionOptions<TItem> {
  /** Extension name (e.g. "slash-command"). */
  name: string;
  /** Trigger character (e.g. "/", "@", ":"). */
  char: string;
  /** Stable plugin key shared with the matching renderer. */
  pluginKey: PluginKey;
  /** Runs when an item is selected. */
  command: (props: SuggestionCommandProps<TItem>) => void;
}

/**
 * Builds a TipTap {@link Extension} wrapping `@tiptap/suggestion` with the
 * shared scaffolding (default suggestion options, the `notInCodeBlock` guard,
 * and the ProseMirror plugin wiring). Each command module configures `items`
 * and `render` later via `.configure({ suggestion: { … } })`.
 */
export function createSuggestionExtension<TItem>(
  options: CreateSuggestionExtensionOptions<TItem>,
): Extension {
  const { name, char, pluginKey, command } = options;

  return Extension.create({
    name,

    addOptions() {
      return {
        suggestion: {
          char,
          command,
          allow: ({ state, range }: { state: EditorState; range: Range }) =>
            notInCodeBlock(state, range),
        } as Partial<SuggestionOptions<TItem, TItem>>,
      };
    },

    addProseMirrorPlugins() {
      return [
        Suggestion({
          pluginKey,
          ...this.options.suggestion,
          editor: this.editor,
        }),
      ];
    },
  });
}
