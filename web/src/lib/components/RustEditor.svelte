<script lang="ts">
	import { onMount, onDestroy } from 'svelte'
	import { EditorView, basicSetup } from 'codemirror'
	import { rust } from '@codemirror/lang-rust'
	import { oneDark } from '@codemirror/theme-one-dark'
	import { EditorState } from '@codemirror/state'

	type Props = {
		value?: string
		onchange?: (value: string) => void
	}

	let { value = '', onchange }: Props = $props()

	let container: HTMLDivElement
	let view: EditorView

	onMount(() => {
		view = new EditorView({
			state: EditorState.create({
				doc: value,
				extensions: [
					basicSetup,
					rust(),
					oneDark,
					EditorView.updateListener.of((update) => {
						if (update.docChanged) {
							onchange?.(update.state.doc.toString())
						}
					}),
					EditorView.theme({
						'&': { borderRadius: '6px', overflow: 'hidden' },
						'.cm-scroller': { fontFamily: "'JetBrains Mono', 'Fira Code', monospace", fontSize: '13px' },
					}),
				],
			}),
			parent: container,
		})
	})

	onDestroy(() => view?.destroy())
</script>

<div bind:this={container} class="editor"></div>

<style>
	.editor {
		border: 1px solid var(--border);
		border-radius: 6px;
		overflow: hidden;
	}

	.editor :global(.cm-editor) {
		max-height: 400px;
	}

	.editor :global(.cm-scroller) {
		overflow: auto;
	}
</style>
