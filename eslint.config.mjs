import antfu from '@antfu/eslint-config'

export default antfu({
  formatters: true,
  ignores: [],
  toml: {
    overrides: {
      'toml/padding-line-between-pairs': 'off',
    },
  },
})
