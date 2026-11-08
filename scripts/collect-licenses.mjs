import { spawnSync } from 'node:child_process'
import { readFile, readdir, mkdir, copyFile, writeFile } from 'node:fs/promises'
import { join, dirname, resolve } from 'node:path'
const cargo=join(process.env.USERPROFILE,'.cargo','bin','cargo.exe')
const result=spawnSync(cargo,['metadata','--manifest-path','native/Cargo.toml','--format-version','1','--filter-platform','x86_64-pc-windows-msvc'],{encoding:'utf8',maxBuffer:32*1024*1024})
if(result.status)throw Error(result.stderr)
const meta=JSON.parse(result.stdout),nodes=new Map(meta.resolve.nodes.map(n=>[n.id,n])),reachable=new Set()
function walk(id){if(reachable.has(id))return;reachable.add(id);for(const dep of nodes.get(id)?.dependencies||[])walk(dep)}walk(meta.resolve.root)
const records=[]
async function collect(ecosystem,name,version,license,root){
 const destination=join('licenses/dependencies',ecosystem,`${name.replaceAll('/','_')}-${version}`)
 const names=(await readdir(root)).filter(n=>/^(licen[cs]e|copying|notice)([.-]|$)/i.test(n))
 await mkdir(destination,{recursive:true})
 const files=[]
 for(const file of names){try{await copyFile(join(root,file),join(destination,file));files.push(file)}catch{}}
 records.push({ecosystem,name,version,license,files})
}
for(const p of meta.packages)if(reachable.has(p.id)&&p.name!=='rove')await collect('rust',p.name,p.version,p.license,dirname(p.manifest_path))
const npmVisited=new Set()
async function npm(name){
 if(npmVisited.has(name))return;npmVisited.add(name)
 const root=resolve('node_modules',name),p=JSON.parse(await readFile(join(root,'package.json'),'utf8'))
 await collect('npm',name,p.version,p.license,root)
 for(const dependency of Object.keys(p.dependencies||{}))await npm(dependency)
}
const p=JSON.parse(await readFile('package.json','utf8'));for(const name of Object.keys(p.dependencies))await npm(name)
await writeFile('licenses/DEPENDENCIES.json',JSON.stringify(records,null,2)+'\n')
console.log(`Collected ${records.length} locked dependency records and ${records.reduce((n,r)=>n+r.files.length,0)} license/notice files.`)
