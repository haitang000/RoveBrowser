// Adapted from Zigzag/Design/SpaceColor.swift (Copyright 2026 Zigao Wang, Apache-2.0).
// Modified: pure JS color extraction, deterministic rounded sRGB output for CSS.
import { writeFile } from 'node:fs/promises'
const decode=x=>x<=.04045?x/12.92:((x+.055)/1.055)**2.4
const encode=x=>x<=.0031308?12.92*x:1.055*x**(1/2.4)-.055
function angle(h){
 const c=h*6,x=1-Math.abs(c%2-1)
 const rgb=(c<1?[1,x,0]:c<2?[x,1,0]:c<3?[0,1,x]:c<4?[0,x,1]:c<5?[x,0,1]:[1,0,x]).map(decode)
 const [r,g,b]=rgb
 const l=Math.cbrt(.4122214708*r+.5363325363*g+.0514459929*b),m=Math.cbrt(.2119034982*r+.6806995451*g+.1073969566*b),s=Math.cbrt(.0883024619*r+.2817188376*g+.6299787005*b)
 return Math.atan2(.0259040371*l+.7827717662*m-.808675766*s,1.9779984951*l-2.428592205*m+.4505937099*s)
}
function tone(h,L,C){
 let rgb
 do{const a=C*Math.cos(h),b=C*Math.sin(h),l=(L+.3963377774*a+.2158037573*b)**3,m=(L-.1055613458*a-.0638541728*b)**3,s=(L-.0894841775*a-1.291485548*b)**3
 rgb=[4.0767416621*l-3.3077115913*m+.2309699292*s,-1.2684380046*l+2.6097574011*m-.3413193965*s,-.0041960863*l-.7034186147*m+1.707614701*s];if(rgb.every(v=>v>=-.0005&&v<=1.0005)||C<=.002)break;C*=.94
 }while(true)
 return '#'+rgb.map(v=>Math.round(encode(Math.min(1,Math.max(0,v)))*255).toString(16).padStart(2,'0')).join('')
}
const hues={sky:.585,sage:.29,sand:.11,rose:.935,lavender:.765,mint:.46,peach:.055}
let css='/* Generated from upstream SpaceColor/OKLab; Copyright 2026 Zigao Wang, Apache-2.0.\n * Modified for Rove. Run node scripts/extract-colors.mjs to reproduce. */\n'
const palette={}
for(const [name,h] of Object.entries(hues)){
 const a=angle(h),k=name==='sky'?.78:1,light=tone(a,.915,.045*k),dark=tone(a,.27,.04*k),degrees=(a*180/Math.PI+360)%360
 palette[name]={light,dark,hue:degrees}
 css+=`:root[data-color="${name}"] { --frame-h: ${degrees.toFixed(6)}; --frame: ${light}; }\n:root[data-theme="dark"][data-color="${name}"] { --frame: ${dark}; }\n.swatches [data-color="${name}"] { background: ${light}; }\n`
}
await writeFile('src/styles/palette.css',css)
await writeFile('docs/color-palette.json',JSON.stringify(palette,null,2)+'\n')
console.log(palette)
