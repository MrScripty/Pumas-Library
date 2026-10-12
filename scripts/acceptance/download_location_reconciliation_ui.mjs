// Actual normal-sandbox Electron workflow on an explicitly authored fixture.
// Node 24 built-in WebSocket drives only this isolated window's CDP target.
import { spawn } from 'node:child_process';
import { readFile, stat, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import assert from 'node:assert/strict';

const [executableArg, fixtureArg, sourceElectronArg] = process.argv.slice(2);
if (!executableArg || !fixtureArg) throw new Error('Expected Electron executable and authored fixture root');
const executable = resolve(executableArg), fixture = resolve(fixtureArg);
const marker = JSON.parse(await readFile(join(fixture, 'pumas-authored-location-fixture.json'), 'utf8'));
assert.deepEqual(marker, {fixtureKind:'pumas.download-location-reconciliation', version:1});
const storePath = join(fixture, 'launcher-data/downloads.json');
const initial = JSON.parse(await readFile(storePath, 'utf8'));
const failure = initial.downloads.find(item => item.repo_id === 'acme/llada-33-files');
assert.equal(failure.status, 'error');
const model = join(fixture, 'shared-resources/models/diffusion/vit/llada-33-files');
const files = failure.filenames.map(name => join(model, name));
const observe = async () => Promise.all(files.map(async file => {
  const info = await stat(file, {bigint:true});
  assert.equal(info.size, 7n, 'Only tiny authored payloads are eligible for this driver');
  return [info.dev.toString(), info.ino.toString(), info.size.toString(), info.mtimeNs.toString()];
}));
const beforeFiles = await observe();

const delay = ms => new Promise(resolveDelay => setTimeout(resolveDelay, ms));
async function connect(port) {
  const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
  const target = targets.find(item => item.type === 'page' && item.url.includes('frontend'));
  if (!target) throw new Error('No isolated frontend target');
  const socket = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolveOpen, reject) => {
    socket.addEventListener('open', resolveOpen, {once:true});
    socket.addEventListener('error', reject, {once:true});
  });
  let sequence = 0;
  const pending = new Map();
  socket.addEventListener('message', event => {
    const message = JSON.parse(event.data);
    const waiter = pending.get(message.id);
    if (!waiter) return;
    pending.delete(message.id);
    if (message.error) waiter.reject(new Error(JSON.stringify(message.error)));
    else waiter.resolve(message.result);
  });
  const call = (method, params = {}) => new Promise((resolveCall, reject) => {
    const id = ++sequence;
    pending.set(id, {resolve:resolveCall, reject});
    socket.send(JSON.stringify({id, method, params}));
  });
  const evaluate = async expression => {
    const result = await call('Runtime.evaluate', {expression, awaitPromise:true, returnByValue:true});
    if (result.exceptionDetails) throw new Error(result.exceptionDetails.text + ': ' + result.exceptionDetails.exception?.description);
    return result.result.value;
  };
  const waitFor = predicate => evaluate(`new Promise((done,reject)=>{
    const check=()=>{if((${predicate})()){observer.disconnect();clearTimeout(timer);done(true);}};
    const observer=new MutationObserver(check);
    const timer=setTimeout(()=>{observer.disconnect();reject(new Error('Expected UI state unavailable: '+(document.body?.innerText??'').slice(0,1800)));},30000);
    observer.observe(document,{subtree:true,childList:true,attributes:true});check();
  })`);
  return {call, evaluate, waitFor, socket};
}

async function session(run) {
  const profile = join(fixture, `ui-profile-${run}`);
  const args = [...(sourceElectronArg ? [resolve(sourceElectronArg)] : []),
    '--launcher-root', fixture, `--user-data-dir=${profile}`, '--remote-debugging-port=0'];
  const child = spawn(executable, args, {env:{...process.env, XDG_CONFIG_HOME:join(fixture,'config'),
    APPDATA:join(fixture,'config'), PUMAS_REGISTRY_DB_PATH:join(fixture,'registry.db')}, stdio:['ignore','pipe','pipe']});
  let logs = '';
  child.stdout.on('data', chunk => {logs += chunk;});
  child.stderr.on('data', chunk => {logs += chunk;});
  const exited = new Promise(resolveExit => child.once('exit', (code,signal) => resolveExit({code,signal})));
  let cdp;
  try {
    const deadline = Date.now()+45000;
    while (Date.now()<deadline) {
      if (child.exitCode !== null) throw new Error(`Electron exited ${child.exitCode}: ${logs.slice(-2200)}`);
      try {
        const port = Number((await readFile(join(profile,'DevToolsActivePort'),'utf8')).split('\n')[0]);
        cdp = await connect(port); break;
      } catch { await delay(100); }
    }
    if (!cdp) throw new Error('Desktop readiness timeout: '+logs.slice(-2200));
    await cdp.call('Page.bringToFront');
    await cdp.waitFor(`()=>document.querySelector('[aria-label="Failed download recovery"]')`);
    // Inspect rendered colors; component/DOM-shim tests cannot prove readability.
    // This local visual oracle requires 7:1 for this dark-theme recovery workflow,
    // not a claim of whole-application accessibility conformance.
    const assertReadable = async (selector, label) => {
      await cdp.waitFor(`()=>{
        const root=document.querySelector(${JSON.stringify(selector)});if(!root)return false;
        for(let node=root;node;node=node.parentElement){
          if(Number(getComputedStyle(node).opacity)!==1)return false;
        }return true;
      }`);
      const observations = await cdp.evaluate(`(()=>{
        const root=document.querySelector(${JSON.stringify(selector)});
        if(!root)throw new Error('Missing visual target');
        const rgb=color=>color.match(/[\\d.]+/g).map(Number);
        const luminance=color=>rgb(color).slice(0,3).map(v=>{
          const n=v/255;return n<=0.04045?n/12.92:((n+0.055)/1.055)**2.4;
        }).reduce((sum,n,i)=>sum+n*[0.2126,0.7152,0.0722][i],0);
        const background=node=>{
          for(let current=node;current;current=current.parentElement){
            const value=getComputedStyle(current).backgroundColor;
            if((rgb(value)[3]??1)===1)return value;
          }throw new Error('No opaque background for visual target');
        };
        return [root,...root.querySelectorAll('p,h2,legend,label,span,button')]
          .filter(node=>node.textContent.trim()&&!node.matches(':disabled')).map(node=>{
            const style=getComputedStyle(node),bg=background(node);
            const fgL=luminance(style.color),bgL=luminance(bg);
            return {text:node.textContent.trim().slice(0,60),color:style.color,background:bg,
              ownBackground:style.backgroundColor,contrast:(Math.max(fgL,bgL)+0.05)/(Math.min(fgL,bgL)+0.05)};
          });
      })()`);
      console.log(label, JSON.stringify(observations));
      for (const item of observations) assert.ok(item.contrast >= 7, `${label}: ${item.text} contrast ${item.contrast.toFixed(2)}:1`);
      if (label !== 'S3 import') assert.match(observations[0].ownBackground, /^rgb\(/, `${label} must have an opaque theme surface`);
    };
    await assertReadable('[aria-label="Failed download recovery"]', 'Recovery banner');
    await cdp.evaluate(`(()=>{
      const button=[...document.querySelectorAll('button')].find(item=>item.textContent.trim()==='Import from S3');
      if(!button)throw new Error('S3 import control missing');button.setAttribute('data-color-probe','s3');
    })()`);
    await assertReadable('[data-color-probe="s3"]', 'S3 import');
    const saveScreenshot = async name => {
      const capture = await cdp.call('Page.captureScreenshot', {format:'png'});
      await writeFile(join(fixture, name), Buffer.from(capture.data,'base64'));
    };
    const assertControlStates = async selector => {
      const point = await cdp.evaluate(`(()=>{
        const box=document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect();
        return {x:box.x+box.width/2,y:box.y+box.height/2};
      })()`);
      await cdp.call('Input.dispatchMouseEvent', {type:'mouseMoved',...point});
      await assertReadable(selector, 'Hovered control');
      await cdp.call('Input.dispatchKeyEvent', {type:'keyDown',key:'Tab',code:'Tab',windowsVirtualKeyCode:9});
      await cdp.call('Input.dispatchKeyEvent', {type:'keyUp',key:'Tab',code:'Tab',windowsVirtualKeyCode:9});
      const focused = await cdp.evaluate(`(()=>{
        const control=document.querySelector(${JSON.stringify(selector)});control.focus();
        const style=getComputedStyle(control);
        return {visible:control.matches(':focus-visible'),width:parseFloat(style.outlineWidth),
          style:style.outlineStyle,color:style.outlineColor,textColor:style.color};
      })()`);
      assert.equal(focused.visible,true,'Keyboard focus must be visible');
      assert.ok(focused.width>=2 && focused.style!=='none','Keyboard focus must have a visible outline');
      await assertReadable(selector, 'Focused control');
    };
    await assertControlStates('[aria-label="Failed download recovery"] button');
    await assertControlStates('[data-color-probe="s3"]');
    if (run === 1) await saveScreenshot('library-colors.png');
    const open = () => cdp.evaluate(`(()=>{
      const section=document.querySelector('[aria-label="Failed download recovery"]');
      const row=[...section.querySelectorAll('div')].find(item=>item.textContent.includes('llada-33-files'));
      if(!row)throw new Error('Exact LLaDA fixture row missing');row.querySelector('button').click();
    })()`);
    if (run === 1) {
      const before = await readFile(storePath);
      await open();
      for (let attempt=0;attempt<32;attempt++) {
        await cdp.waitFor(`()=>document.querySelector('[role="dialog"]')?.textContent.includes('33 selected files')||document.querySelector('[role="dialog"]')?.textContent.includes('outstanding work or custody')`);
        if (await cdp.evaluate(`document.querySelector('[role="dialog"]').textContent.includes('33 selected files')`)) break;
        await cdp.evaluate(`[...document.querySelector('[role="dialog"]').querySelectorAll('button')].find(button=>button.textContent==='Inspect again').click()`);
      }
      assert.equal(await cdp.evaluate(`document.querySelector('[role="dialog"]').textContent.includes('33 selected files')`),true);

      assert.equal(await cdp.evaluate(`document.querySelector('[role="dialog"]').textContent.includes('Older applications that open the library directly')`), true);
      assert.equal(await cdp.evaluate(`document.querySelector('[role="dialog"]').textContent.includes('does not verify their historical source')`), true);
      await assertReadable('[role="dialog"]', 'Recovery dialog');
      await assertControlStates('[role="dialog"] button:not(:disabled)');
      await saveScreenshot('recovery-dialog-colors.png');
      await cdp.evaluate(`[...document.querySelector('[role="dialog"]').querySelectorAll('button')].find(button=>button.textContent==='Cancel').click()`);
      await cdp.waitFor(`()=>!document.querySelector('[role="dialog"]')`);
      assert.deepEqual(await readFile(storePath), before, 'Cancel must leave storage unchanged');
      await open();
      await cdp.waitFor(`()=>[...document.querySelector('[role="dialog"]')?.querySelectorAll('button')??[]].some(button=>button.textContent==='Keep existing files and resolve failure'&&!button.disabled)`);
      await cdp.evaluate(`[...document.querySelector('[role="dialog"]').querySelectorAll('button')].find(button=>button.textContent==='Keep existing files and resolve failure').focus()`);
      await cdp.call('Input.dispatchKeyEvent', {type:'rawKeyDown', key:'Enter', code:'Enter', windowsVirtualKeyCode:13});
      await cdp.call('Input.dispatchKeyEvent', {type:'char', key:'Enter', text:'\r', unmodifiedText:'\r', windowsVirtualKeyCode:13});
      await cdp.call('Input.dispatchKeyEvent', {type:'keyUp', key:'Enter', code:'Enter', windowsVirtualKeyCode:13});
      await cdp.waitFor(`()=>document.querySelector('[role="dialog"]')?.textContent.includes('old failed download was resolved')`);
      await cdp.waitFor(`()=>[...document.querySelector('[role="dialog"]')?.querySelectorAll('button')??[]].some(button=>button.textContent==='Close'&&!button.disabled)`);
      await assertReadable('[role="dialog"]', 'Resolved recovery dialog');
      await cdp.evaluate(`[...document.querySelector('[role="dialog"]').querySelectorAll('button')].find(button=>button.textContent==='Close').click()`);
      await cdp.waitFor(`()=>!document.querySelector('[role="dialog"]')&&!document.querySelector('[aria-label="Failed download recovery"]').textContent.includes('llada-33-files')`);
      assert.deepEqual(await observe(), beforeFiles);
      console.log('PASS: normal Electron renderer/preload/IPC/RPC keeps 33 files, warns before confirmation, cancels without writing, and resolves via keyboard in-session');
    } else {
      assert.equal(await cdp.evaluate(`document.querySelector('[aria-label="Failed download recovery"]').textContent.includes('llada-33-files')`), false);
      assert.deepEqual(await observe(), beforeFiles);
      const data = JSON.parse(await readFile(storePath,'utf8'));
      assert.equal(data.retained_download_resolutions[failure.download_id].original.status, 'error');
      assert.equal(data.downloads.filter(item=>item.status==='paused').length,2);
      console.log('PASS: real desktop close/reopen preserves history, file identities and unrelated paused downloads');
    }
    await cdp.call('Page.close');
    const outcome = await Promise.race([exited, delay(20000).then(()=>{throw new Error('Electron clean shutdown timeout');})]);
    assert.equal(outcome.code,0,'Normal desktop must exit cleanly');
  } finally {
    if (child.exitCode === null && cdp) await Promise.race([cdp.call('Page.close').catch(()=>undefined),exited,delay(5000)]);
    cdp?.socket.close();
    if (child.exitCode === null) { child.kill('SIGTERM'); await exited; }
  }
}
await session(1);
await session(2);
