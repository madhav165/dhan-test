import adapter from '@sveltejs/adapter-auto';
import { relative, sep } from 'node:path';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	compilerOptions: {
		runes: ({ filename }) => {
			const relativePath = relative(import.meta.dirname, filename);
			const pathSegments = relativePath.toLowerCase().split(sep);
			return pathSegments.includes('node_modules') ? undefined : true;
		}
	},
	kit: {
		adapter: adapter()
	}
};

export default config;
