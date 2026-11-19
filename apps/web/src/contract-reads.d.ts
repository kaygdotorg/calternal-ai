/** The ids of the operations the contract marks `mutates: false` (see `contractReads` in vite.config.ts). */
declare module "virtual:contract-reads" {
  const ids: string[];
  export default ids;
}
