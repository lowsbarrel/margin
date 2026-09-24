import type { Completion } from '@codemirror/autocomplete';
import { EditorView } from '@codemirror/view';

export interface AssistCompletion extends Completion {
	assistIcon: string;
	assistDescription: string;
}

export function assistCompletion(
	label: string,
	description: string,
	icon: string,
	extra: Partial<Completion>
): AssistCompletion {
	return { ...extra, label, assistIcon: icon, assistDescription: description };
}

function isAssist(completion: Completion): completion is AssistCompletion {
	return 'assistIcon' in completion;
}

const OPTION_CLASS = 'cm-assist-option';

export const assistCompletionConfig = {
	icons: false,
	optionClass: (completion: Completion) => (isAssist(completion) ? OPTION_CLASS : ''),
	tooltipClass: () => 'cm-assist-tooltip',
	addToOptions: [
		{
			position: 10,
			render: (completion: Completion) => {
				if (!isAssist(completion) || !completion.assistIcon) return null;
				const icon = document.createElement('span');
				icon.className = 'cm-assist-icon';
				icon.textContent = completion.assistIcon;
				return icon;
			}
		},
		{
			position: 90,
			render: (completion: Completion) => {
				if (!isAssist(completion) || !completion.assistDescription) return null;
				const description = document.createElement('span');
				description.className = 'cm-assist-desc';
				description.textContent = completion.assistDescription;
				return description;
			}
		}
	]
};

export const assistTheme = EditorView.theme({
	'& .cm-tooltip.cm-assist-tooltip': {
		background: 'var(--color-bg-primary)',
		border: '1px solid var(--color-border-secondary)',
		borderRadius: 'var(--radius-md)',
		boxShadow: 'var(--shadow-lg)',
		padding: '4px',
		maxHeight: '320px',
		overflow: 'hidden',
		color: 'var(--color-text-primary)',
		fontFamily: 'var(--font-sans)'
	},
	'& .cm-tooltip.cm-assist-tooltip > ul': {
		fontFamily: 'var(--font-sans)',
		minWidth: '260px',
		maxWidth: 'min(340px, 90vw)',
		maxHeight: '312px',
		height: 'auto',
		whiteSpace: 'nowrap'
	},
	'& .cm-tooltip.cm-assist-tooltip ul > li': {
		display: 'grid',
		gridTemplateAreas: '"icon label" "icon desc"',
		gridTemplateColumns: 'auto 1fr',
		alignItems: 'center',
		columnGap: '10px',
		padding: '5px 8px',
		lineHeight: '1.3',
		borderRadius: 'var(--radius-sm)',
		overflowX: 'hidden'
	},
	'& .cm-tooltip.cm-assist-tooltip ul > li[aria-selected]': {
		background: 'var(--color-bg-secondary)',
		color: 'var(--color-text-primary)'
	},
	'& .cm-tooltip.cm-assist-tooltip .cm-completionLabel': {
		gridArea: 'label',
		fontSize: 'var(--text-size-sm)',
		fontWeight: 'var(--font-weight-medium)',
		color: 'var(--color-text-primary)',
		overflow: 'hidden',
		textOverflow: 'ellipsis'
	},
	'& .cm-tooltip.cm-assist-tooltip .cm-completionMatchedText': {
		textDecoration: 'none',
		fontWeight: 'var(--font-weight-semibold)',
		color: 'var(--color-text-brand)'
	},
	'& .cm-tooltip.cm-assist-tooltip .cm-assist-icon': {
		gridArea: 'icon',
		display: 'flex',
		alignItems: 'center',
		justifyContent: 'center',
		width: '30px',
		height: '30px',
		borderRadius: 'var(--radius-sm)',
		background: 'var(--color-bg-secondary)',
		border: '1px solid var(--color-border-secondary)',
		color: 'var(--color-text-secondary)',
		fontSize: 'var(--text-size-sm)'
	},
	'& .cm-tooltip.cm-assist-tooltip ul > li[aria-selected] .cm-assist-icon': {
		background: 'var(--color-brand-12)',
		borderColor: 'var(--color-brand-24)',
		color: 'var(--color-text-brand)'
	},
	'& .cm-tooltip.cm-assist-tooltip .cm-assist-desc': {
		gridArea: 'desc',
		fontSize: 'var(--text-size-xs)',
		color: 'var(--color-text-tertiary)',
		overflow: 'hidden',
		textOverflow: 'ellipsis'
	}
});
