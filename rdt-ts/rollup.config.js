import resolve from '@rollup/plugin-node-resolve';
import typescript from '@rollup/plugin-typescript';
import dts from 'rollup-plugin-dts';

const config = [
    // ES Module build
    {
        input: 'src/index.ts',
        output: {
            file: 'dist/index.esm.js',
            format: 'es',
            sourcemap: true
        },
        plugins: [
            resolve(),
            typescript({ tsconfig: './tsconfig.json' })
        ],
        external: ['zustand', 'lib0']
    },
    // CommonJS build
    {
        input: 'src/index.ts',
        output: {
            file: 'dist/index.js',
            format: 'cjs',
            sourcemap: true
        },
        plugins: [
            resolve(),
            typescript({ tsconfig: './tsconfig.json' })
        ],
        external: ['zustand', 'lib0']
    },
    // Type definitions
    {
        input: 'dist/index.d.ts',
        output: {
            file: 'dist/index.d.ts',
            format: 'es'
        },
        plugins: [dts()]
    }
];

export default config; 