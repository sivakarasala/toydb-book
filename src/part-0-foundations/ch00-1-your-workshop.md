# Your Workshop — Terminal, Files & Editors

Before you build anything, you need a workspace. A carpenter has a workbench, a hammer, and a saw. A programmer has a **terminal**, a **code editor**, and **files**. This chapter will introduce you to all three.

By the end, you will be able to navigate your computer using text commands, create files and folders, and open them in a professional code editor. These are the foundational skills you will use every single day as a programmer.

Take your time here. None of this is difficult, but all of it is new. That is okay.

---

## What Is a Program?

You already use programs every day. Your weather app, your calculator, even the clock on your phone — these are all programs.

A **program** is a set of instructions that tells a computer what to do. Think of it like a recipe:

- A recipe says: *"Preheat the oven to 375 degrees. Mix flour and sugar. Bake for 25 minutes."*
- A program says: *"Open a window on the screen. Show the user's name. Wait for them to click a button."*

The recipe is written in English so a human can follow it. A program is written in a **programming language** so a computer can follow it.

Throughout this book, we are going to write a program called **ToyDB** — a database built from scratch. It will let you store data, query it, and understand how real databases work under the hood. But first, we need to set up the workshop where we will build it.

---

## The Terminal

Most of the time, you interact with your computer by clicking icons, dragging windows, and tapping buttons. That is called a **graphical user interface** (GUI, pronounced "gooey").

There is another way to talk to your computer: by typing text commands. The program that lets you do this is called a **terminal** (sometimes called a "command line" or "shell"). It looks like a plain text window with a blinking cursor, and it is one of the most powerful tools a programmer has.

Why use the terminal when you could just click around? Because typing commands is **precise**, **repeatable**, and **fast**. When you tell a friend how to find a file, you can say *"Open your home folder, then go into Documents, then open the Projects folder"* — or you can just say: `cd ~/Documents/Projects`. One line. No ambiguity.

### How to open the terminal

**macOS:**
1. Press `Cmd + Space` to open Spotlight search.
2. Type `Terminal` and press Enter.
3. A window will appear with a blinking cursor. That is your terminal.

**Linux (Ubuntu, Fedora, etc.):**
1. Press `Ctrl + Alt + T`.
2. A terminal window will appear.

**Windows:**
Windows does not come with a Unix-style terminal by default, but you can install one called **WSL** (Windows Subsystem for Linux):
1. Open PowerShell **as Administrator** (right-click the Start button, select "Windows Terminal (Admin)").
2. Type: `wsl --install` and press Enter.
3. Restart your computer when prompted.
4. After restart, open "Ubuntu" from your Start menu. This is your terminal.

> If you are on Windows and the WSL steps feel overwhelming right now, don't worry. You can also follow along using PowerShell for most of this chapter. We will make sure WSL is set up before we start writing Rust code.

Go ahead and open your terminal now. You should see something like this:

```
yourname@yourcomputer ~ %
```

or

```
yourname@yourcomputer:~$
```

The exact format varies, but you will see your username, your computer's name, and a symbol (`%` or `$`) that means *"I am ready for your command."* This is called the **prompt**.

---

## Basic Commands

Let's learn six commands. These are the ones you will use most often. Type each one into your terminal and press Enter to run it.

### `pwd` — Print Working Directory

```bash
pwd
```

This tells you **where you are** right now in your computer's file system. Think of your computer's files as a big tree of folders. `pwd` says *"Which folder am I standing in?"*

You will see something like:

```
/Users/yourname
```

or on Linux:

```
/home/yourname
```

This is your **home directory** — the folder that belongs to you. The `/` characters separate folder names, like a path through the tree. `/Users/yourname` means: start at the root of the computer (`/`), go into the `Users` folder, then go into the `yourname` folder.

### `ls` — List

```bash
ls
```

This shows you **what is inside** the current folder. You might see things like:

```
Desktop    Documents    Downloads    Music    Pictures
```

These are the folders (and files) that live in your home directory. It is like opening a folder in Finder or File Explorer and seeing what is inside.

### `cd` — Change Directory

```bash
cd Documents
```

This **moves you into** a different folder. After running this, your prompt will change to show you are inside `Documents`, and if you run `pwd` again, you will see:

```
/Users/yourname/Documents
```

To go **back up** one level (to the parent folder), type:

```bash
cd ..
```

The `..` is a special shortcut that means "the folder above this one." And there is another shortcut: `~` (called "tilde") always means your home directory. So no matter where you are, you can always get home with:

```bash
cd ~
```

### `mkdir` — Make Directory

```bash
mkdir my_data
```

This **creates a new folder** called `my_data` in whatever folder you are currently in. You will not see any output — silence means success. Run `ls` afterwards to confirm the folder was created.

### `touch` — Create a File

```bash
touch notes.txt
```

This **creates a new, empty file** called `notes.txt`. Again, no output means it worked. Run `ls` to see it appear.

### `cat` — Display a File's Contents

```bash
cat notes.txt
```

This **shows the contents** of a file right in the terminal. Since `notes.txt` is empty, you will see nothing. Soon we will put text into it and `cat` will become more useful.

> **Why these names?** Terminal commands come from the early days of computing (the 1970s). `ls` is short for "list." `cd` is "change directory." `cat` is short for "concatenate," which means to join things together — it was originally designed to combine multiple files, but people mostly use it to read single files. The names are short because programmers type them hundreds of times a day.

---

## Your First Directory

Let's create the workspace where all of our ToyDB projects will live. Type each command one at a time:

```bash
cd ~
```

This takes you to your home directory (you might already be there, but it doesn't hurt to make sure).

```bash
mkdir rusty
```

This creates a folder called `rusty`. We are learning Rust, so this is where all our Rust projects will live.

```bash
cd rusty
```

Now you are inside the `rusty` folder. Verify with:

```bash
pwd
```

You should see:

```
/Users/yourname/rusty
```

Congratulations — you just created your first programming workspace using nothing but text commands.

---

## Code Editors

You *could* write code using the basic text editor that comes with your computer (like Notepad on Windows or TextEdit on macOS). But that would be like trying to build a house with a butter knife. It will technically cut things, but it is not the right tool for the job.

A **code editor** is a program designed specifically for writing code. It gives you:

- **Syntax highlighting** — different parts of your code appear in different colors, making it easier to read (like how a recipe might bold the ingredient names).
- **Auto-completion** — it suggests what you might want to type next, saving time and preventing typos.
- **Error detection** — it underlines mistakes before you even run your code, like a spell-checker for programming.
- **An integrated terminal** — a terminal built right into the editor, so you do not have to switch windows.

### Installing VS Code

We recommend **Visual Studio Code** (VS Code) — it is free, works on every operating system, and is the most popular code editor in the world.

1. Go to [https://code.visualstudio.com](https://code.visualstudio.com).
2. Click the big download button. It will detect your operating system automatically.
3. Install it the same way you install any other program:
   - **macOS:** Open the `.dmg` file, drag VS Code to your Applications folder.
   - **Windows:** Run the `.exe` installer, accept the defaults.
   - **Linux:** Follow the instructions on the download page for your distribution.
4. Open VS Code. You will see a Welcome tab. You can close it.

### Opening a folder in VS Code

In VS Code, you do not open individual files — you open **folders**. This lets the editor understand the structure of your project.

You can do this from the terminal:

```bash
code ~/rusty
```

This tells VS Code to open the `rusty` folder. If the `code` command does not work:
- **macOS:** Open VS Code, press `Cmd + Shift + P`, type "shell command", and select *"Install 'code' command in PATH"*. Then try again.
- **Windows/Linux:** The installer usually sets this up automatically. Try restarting your terminal.

You can also open a folder from inside VS Code: click **File > Open Folder** and navigate to `~/rusty`.

---

## File Types

When you look at files on your computer, you will notice they have **extensions** — the part after the last dot in the filename. For example, `photo.jpg`, `essay.docx`, `song.mp3`. The extension tells you (and the computer) what kind of file it is.

In this book, you will work with a few specific file types:

| Extension | What it is | Example |
|-----------|-----------|---------|
| `.rs` | A **Rust source file** — this is where you write your Rust code | `main.rs` |
| `.toml` | A **configuration file** (Tom's Obvious, Minimal Language) — used to describe your project's settings | `Cargo.toml` |
| `.md` | A **Markdown file** — a simple way to write formatted text (this book is written in Markdown!) | `README.md` |
| `.txt` | A **plain text file** — just text, no formatting | `notes.txt` |
| `.sql` | A **SQL file** — commands for databases (you will write plenty of these) | `schema.sql` |

You don't need to memorize these. You will get familiar with them naturally as we build ToyDB.

---

## Exercises

### Exercise 1: Where Am I?

**Goal:** Open the terminal and find out where you are in the file system.

**Instructions:**
1. Open your terminal (see the instructions earlier in this chapter).
2. Type `pwd` and press Enter.
3. Look at the output carefully.

<details>
<summary>Hints</summary>

- On macOS, open Spotlight with `Cmd + Space`, type "Terminal", and press Enter.
- On Linux, press `Ctrl + Alt + T`.
- On Windows, open the Ubuntu app (if you installed WSL) or PowerShell.
- The output of `pwd` will be a path like `/Users/yourname` or `/home/yourname`.

</details>

**What you should see:**

A path that ends with your username. This is your home directory — your personal space on the computer. Every time you open a new terminal, this is where you start.

```
/Users/yourname
```

---

### Exercise 2: Build a Playground

**Goal:** Create a folder, create a file inside it, write text to the file, and read it back.

**Instructions:**
1. Make sure you are in your home directory: `cd ~`
2. Create the `rusty` directory (if you haven't already): `mkdir rusty`
3. Move into it: `cd rusty`
4. Create a `playground` directory: `mkdir playground`
5. Move into it: `cd playground`
6. Create a file with a message inside it:
   ```bash
   echo "Hello ToyDB" > hello.txt
   ```
7. Read the file: `cat hello.txt`

<details>
<summary>Hints</summary>

- If you get `mkdir: cannot create directory 'rusty': File exists`, that just means you already created it. That is fine — move on to the next step.
- The `echo` command prints text, and the `>` symbol redirects that text into a file instead of showing it on screen. Think of `>` as an arrow pointing from the text into the file.
- If you make a typo, you can press the **Up arrow** key to bring back your last command, edit it, and press Enter again.

</details>

**What you should see:**

After step 7, the terminal should display:

```
Hello ToyDB
```

You just created a file and read it back — entirely from the command line. That is essentially what a database does: store data and retrieve it on demand. We are just doing it by hand right now.

---

### Exercise 3: Meet Your Editor

**Goal:** Install VS Code, open your playground folder, and edit a file with a real code editor.

**Instructions:**
1. Download VS Code from [https://code.visualstudio.com](https://code.visualstudio.com) and install it.
2. Open VS Code.
3. Open your playground folder using one of these methods:
   - From the terminal: `code ~/rusty/playground`
   - From VS Code: **File > Open Folder**, then navigate to your `rusty/playground` folder.
4. In the left sidebar, you should see `hello.txt`. Click on it.
5. Edit the file. Change it to say: `Hello ToyDB! I'm ready to learn Rust.`
6. Save the file: `Cmd + S` (macOS) or `Ctrl + S` (Windows/Linux).
7. Go back to your terminal and run: `cat ~/rusty/playground/hello.txt`

<details>
<summary>Hints</summary>

- If the `code` command does not work on macOS, open VS Code, press `Cmd + Shift + P`, type "shell command", and select *"Install 'code' command in PATH."*
- The left sidebar in VS Code is called the **Explorer**. It shows all the files and folders in your project.
- When a file has unsaved changes, VS Code shows a dot next to its name in the tab bar.

</details>

**What you should see:**

In the terminal, `cat` should now display your updated message:

```
Hello ToyDB! I'm ready to learn Rust.
```

You just edited a file in a professional code editor and confirmed the change from the terminal. The editor and the terminal are looking at the same files — they are just two different ways to interact with them.

---

### Exercise 4: Navigate Like a Pro

**Goal:** Practice moving between directories using `cd` until it feels natural.

**Instructions:**
1. Open your terminal.
2. Go to your home directory: `cd ~`
3. Verify: `pwd` (should show your home directory)
4. Navigate to your playground: `cd rusty/playground`
5. Verify: `pwd` (should show `~/rusty/playground`)
6. Go back to your home directory: `cd ~`
7. Verify: `pwd` (should show your home directory again)
8. Navigate to playground again, but this time using the **full path**: `cd ~/rusty/playground`
9. Go up one level: `cd ..`
10. Verify: `pwd` (should show `~/rusty`)
11. Go up one more level: `cd ..`
12. Verify: `pwd` (should show your home directory)

<details>
<summary>Hints</summary>

- `cd rusty/playground` is a **relative path** — it means "from where I am now, go into `rusty`, then into `playground`." It only works if you are in the right starting location.
- `cd ~/rusty/playground` is an **absolute path** — it means "start from my home directory, go into `rusty`, then into `playground`." It works no matter where you are.
- `cd ..` always means "go up one level."
- If you get lost at any point, `cd ~` will always take you home.

</details>

**What you should see:**

Each time you run `pwd`, the output matches the directory you expect to be in. Here is the full sequence:

```
/Users/yourname
/Users/yourname/rusty/playground
/Users/yourname
/Users/yourname/rusty/playground
/Users/yourname/rusty
/Users/yourname
```

If your output matches this pattern (with your actual username), you have mastered terminal navigation. This is a skill you will use every day.

---

## Summary

Here is what you learned in this chapter:

| Concept | What it means |
|---------|--------------|
| **Program** | A set of instructions for a computer, like a recipe |
| **Terminal** | A text-based way to control your computer |
| **`pwd`** | Print where you are |
| **`ls`** | List what is in the current folder |
| **`cd`** | Change to a different folder |
| **`mkdir`** | Create a new folder |
| **`touch`** | Create a new empty file |
| **`cat`** | Display a file's contents |
| **Code editor** | A program designed for writing code (we use VS Code) |
| **File extension** | The part after the dot (`.rs`, `.toml`, etc.) that indicates the file type |

You now have a workspace (`~/rusty`), a terminal you can navigate, and a code editor ready to go. In the next chapter, we will install Rust and write your very first program.
