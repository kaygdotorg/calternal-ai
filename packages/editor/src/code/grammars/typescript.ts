// Loads the typescript syntax parser on its own, so a code block pulls it in only when that language is used.
import { parser } from '@lezer/javascript'
const parserTs = parser.configure({ dialect: 'ts jsx' })
export { parserTs as parser }
