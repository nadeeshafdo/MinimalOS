---
layout: default
title: MinimalOS — A Rust OS from Scratch
---

<div style="margin-top:32px;margin-bottom:32px;text-align:left;">
  <img src="assets/logo.svg" alt="MinimalOS logo" style="width:56px;height:56px;vertical-align:middle;margin-right:18px;float:left;">
  <h1 style="font-size:1.5rem;font-weight:600;margin:0 0 0.2em 0;line-height:1.2;">MinimalOS</h1>
  <div style="font-size:1.02rem;color:var(--text-muted);margin-bottom:0.5em;">A 64-bit x86_64 operating system kernel written in Rust from scratch</div>
</div>

<hr style="border:0;border-top:1px solid var(--border);margin:18px 0 28px 0;">

<h2 style="font-size:1.1rem;font-weight:600;margin:0 0 0.7em 0;">Quick Start</h2>
<pre style="background:var(--bg-code);color:var(--text);border:1px solid var(--border);padding:0.9em 1.2em;font-size:0.98em;overflow-x:auto;">git clone https://github.com/nadeeshafdo/MinimalOS.git
cd MinimalOS
make iso
make run</pre>
<div style="font-size:0.97em;color:var(--text-muted);margin-bottom:1.5em;">Requires Rust nightly (see <code>rust-toolchain.toml</code>), QEMU, xorriso, git, make.</div>

<h2 style="font-size:1.1rem;font-weight:600;margin:2.2em 0 0.7em 0;">Documentation</h2>
<ul style="list-style:none;padding:0;margin:0 0 1.5em 0;">
  <li><a href="kernel_architecture">Kernel Architecture</a></li>
  <li><a href="memory_management">Memory Management</a></li>
  <li><a href="process_management">Process Management</a></li>
  <li><a href="syscalls">Syscall Reference</a></li>
  <li><a href="userspace">Userspace Guide</a></li>
  <li><a href="drivers">Drivers</a></li>
  <li><a href="development_guide">Development Guide</a></li>
</ul>

<h2 style="font-size:1.1rem;font-weight:600;margin:2.2em 0 0.7em 0;">Project Overview</h2>
<ul style="margin:0 0 1.5em 0;padding-left:1.1em;">
  <li>Bitmap PMM, 4-level paging, kernel heap</li>
  <li>Preemptive multitasking, round-robin scheduler, APIC timer</li>
  <li>User mode (Ring 3), syscalls, GDT/TSS, dynamic RSP0</li>
  <li>USTAR ramdisk, ELF loader, userspace shell</li>
  <li>Framebuffer console, bitmap fonts, color, scrolling</li>
</ul>

<h2 style="font-size:1.1rem;font-weight:600;margin:2.2em 0 0.7em 0;">Development Roadmap</h2>
<table style="width:100%;border-collapse:collapse;font-size:0.98em;">
  <thead>
	<tr style="border-bottom:1px solid var(--border);">
	  <th style="text-align:left;padding:0.3em 0.5em 0.3em 0;">Rank</th>
	  <th style="text-align:left;padding:0.3em 0.5em;">Focus</th>
	  <th style="text-align:left;padding:0.3em 0.5em;">Status</th>
	</tr>
  </thead>
  <tbody>
	<tr><td>I</td><td>The Awakening — Boot & Basics</td><td>Complete</td></tr>
	<tr><td>II</td><td>The Artist — Graphics & Output</td><td>Complete</td></tr>
	<tr><td>III</td><td>The Reflexes — Interrupts & CPU</td><td>Complete</td></tr>
	<tr><td>IV</td><td>The Mind — Memory Management</td><td>Complete</td></tr>
	<tr><td>V</td><td>The Senses — Input & Drivers</td><td>Complete</td></tr>
	<tr><td>VI</td><td>The Barrier — User Mode & Syscalls</td><td>Complete</td></tr>
	<tr><td>VII</td><td>The Vault — Storage & Files</td><td>Complete</td></tr>
	<tr><td>VIII</td><td>The Conductor — Multitasking & IPC</td><td>Complete</td></tr>
	<tr><td>IX</td><td>The Network — Data & Buses</td><td>Next</td></tr>
  </tbody>
</table>
<div style="font-size:0.93em;color:var(--text-muted);margin:0.7em 0 1.5em 0;">
  See <a href="https://github.com/nadeeshafdo/MinimalOS/blob/main/QUESTS.md">QUESTS.md</a> for the full tracker.
</div>

<hr style="border:0;border-top:1px solid var(--border);margin:32px 0 18px 0;">
<div style="font-size:0.92em;color:var(--text-muted);text-align:left;">
  MinimalOS is an educational project — free to use and modify.<br>
  <a href="https://github.com/nadeeshafdo/MinimalOS">View on GitHub</a>
</div>
