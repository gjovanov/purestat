import typescript from '@rollup/plugin-typescript';
import terser from '@rollup/plugin-terser';

export default {
  input: 'src/purestat.ts',
  output: {
    file: 'dist/purestat.js',
    format: 'iife',
    sourcemap: false,
  },
  plugins: [
    typescript(),
    terser({
      compress: {
        drop_console: true,
        passes: 2,
      },
      mangle: true,
    }),
  ],
};
