# Plotting a sheet

How the screensaver's pen plots a sheet, and why it plots it so. Each sheet's
drawing is turned into the work of a pen once, on its first frame, and every
frame looks its moment up: the drawing goes down as a drafting office draws
it, at the pace of a draughtsman's hand played a little over twice as fast,
and the title block fills in as the drawing reaches what it says. The code is
in `layershell/crates/screensaver/src/sheet/plotter/`; how a sheet goes as a
whole is in [`screensaver.md`](screensaver.md).

Units throughout are virtual pixels and seconds. The hand's figures are for
the laptop's sheet, 853 × 533 virtual pixels; on another output every length
and speed is multiplied by the sheet's height over 533 (1.35 on the 1720 × 720
ultrawide), so a drawing takes as long everywhere. The screensaver runs at 30
frames a second, so a frame is 33 ms, and every timing below is chosen to read
at that rate.

## What makes a plot worth watching

- **It is a process to follow.** People who own pen plotters say they like
  watching one work as much as they like what it draws [1], and a slow plotter
  holds the eye better than a fast one [2]. What people watch is the pen at
  work more than the picture appearing.
- **Ink grows only part of the time.** A real plotter, an HP 7475A or 7550A,
  an AxiDraw or a GRBL machine, puts ink down for between a third and a half
  of its time; the rest is darting with the pen up, dropping it and lifting it
  [3][4][5][6]. Ink that grows all the time at one rate reads as a reveal, not
  a machine.
- **A visible hand carries the attention.** A drawing made in front of viewers
  teaches them more than a finished one, but only while they can see the hand
  making it [7]. The pen's head is that hand, and it is on every frame of the
  plot.
- **Rhythm comes from contrast.** Long edges sweep and ease, while lettering,
  dashes and hatching peck. Real controllers do this on purpose: Calcomp's
  dual-mode acceleration steps short vectors and ramps long ones [8], and HP
  kept the pen low between letters because lifting it had come to take a large
  share of the time text took to plot [9].
- **The order makes sense and the pace can be followed.** Viewers take in the
  whole before its parts [10], and people sketch the long, salient strokes
  first [11]. A believable drawing order goes on to the nearest element and
  sweeps consistently [12]. Draughtsmen lay down centre lines, then the
  shapes, then the dimensions and the words [13]. An animation fails when it
  is too complex or too fast to follow [14], which a fixed plot length makes a
  dense sheet.
- **It is honest.** A pen never erases and never jumps, and the plot visibly
  finishes, the pen put away, before the machine starts to run.

## How a plot is worked out

On a sheet's first frame, before the pen starts at 0.6 s:

1. Every mark of every view is rasterized into pieces, as the run's first
   frame draws them.
2. Each lit pixel is given one owner, the piece that paints it last in the
   run's order ([ownership](#ownership)).
3. The pieces become strokes, each a motion of the pen ([strokes](#strokes)).
4. The strokes are put in a drafting office's order ([order](#order)).
5. The pen's moves along and between them are planned as a hand moves
   ([motion](#motion)), and the plot is as long as they take at the hand's
   pace ([pacing](#pacing)).

The plot depends only on the subject, which showing of it this is, the
output's size and how long the schedule gave its plot, so every frame of it is
a function of the time: there is no unseeded randomness, no iteration over a
hash map, every sort key ends in stroke and mark indices, and time is
accumulated in a fixed order. A frame finds its moment by a binary search over
the pen's moves and works it out in closed form. A live sheet's drawing is
frozen as it stood when its plot was worked out, so the kept drawings of its
plot cannot change under it.

### Ownership

Every piece is painted once into an owner buffer, a number per pixel, in the
run's order: what stays still, pass by pass, then what moves over it. The
ground a label clears is painted too, with the label as its owner. A stroke
inks only the pixels it owns, so the strokes never overlap, and:

- whatever order they are drawn in, they finish on exactly the drawing the
  subject's run begins with, so the last frame of the plot is the run's first;
- nothing is ever drawn over or wiped: the ground under lettering is a gap the
  pen leaves, as a draughtsman leaves it, and the pen passes through it.

What moves along the drawing's traces (traffic on the buses, flow, a signal)
is not plotted: it appears when the subject starts to run, as the machine
switching on, and the pixels it would cover are plotted. The same buffer says
which marks touch, pixel beside pixel, which a diagram grows along.

### Strokes

A stroke is one motion of the pen: the pixels it passes over in order, which
of them it inks (its line type's pattern and what it owns, worked out once in
the direction it was made and carried with the pixels, so turning a stroke
round or starting a loop elsewhere never moves a dash), its pen and what it
draws. Strokes go in groups, a mark's, which keep their order: a dimension's
extension lines, its line, its arrowheads, its value.

| Piece | Becomes |
|---|---|
| A line of any type | One stroke, from the first pixel inside the drawing to the last. A dashed or chain line is one carriage motion with the pen bouncing over the gaps, as HP-GL's line types draw them [15], never a stroke a dash. One that ends where it began, or beside it, is a loop, which may be started anywhere. |
| Section lining | A stroke along each of its diagonals, back and forth across the area: each lit pixel on its 45° line, each line cut where it leaves the area, alternate lines drawn the other way [16][17]. No connectors are drawn; the pen is carried between them. |
| An area of solid or tint | One stroke back and forth along its rows, inking the pixels its texture lights. |
| Lettering | The strokes of each glyph, letter by letter and never turned round, as lettering is written [18][15]. Each glyph of the pixel font is traced once into the fewest trails a greedy walk finds, each as straight as it can go and starting at a free end, the leftmost then the highest: a stroke for each pair of free ends, one for a ring. |
| A dot | A touch: the pen down and up. |
| The ground a label clears | Nothing: a gap the pen leaves. |

### Order

Groups are drawn in the order of their keys, the drafting office's stages,
after French's orders of pencilling and inking [13], Rose [19], Armengaud
[20], the Navy's order of inking [21] and the order of a CAD section drawing
[22]:

| Stage | What | In what order |
|---|---|---|
| Skeleton | Centre and phantom lines and construction belonging to no part, in every view together | The axes first, straight lines at least half as long as the longest, longest first; then the rest nearest first |
| Bodies | Outlines, a part's own construction and its hidden lines, view by view, the front view first; a cutting plane, its arrows and letters before the section it cuts | What belongs to no part first, then the parts in the order of their items, the main or driving part first, so pinion before gear and crank before rod, the chain of cause a viewer follows working a machine out [23]. Within a part its circles, arcs and closed outlines before its straight lines (circles are inked first [13]), circles round one centre one after another and smallest first, as a compass is opened out [20], then the lines hidden behind it |
| Lining | Hatching, cross-hatching, fills | Part by part, back and forth |
| Annotation | Dimensions, notes, finish, datums and control frames, labels | View by view, nearest first |
| Traces | What the drawing traces and does not move | Nearest first |
| Balloons | In the order of their items | The dot on the part, the leader out from it, the circle from where the leader meets it, then the number |

Between stages the pen hovers 150 ms, and 60 ms between the parts or views a
stage keeps apart.

**Diagrams grow.** On a sheet not drawn to scale, a block diagram or a
schematic, the bodies grow from the first part (the processor, or the 555)
outwards along what touches it, breadth first. As the walk reaches a part, the
whole part is drawn straight after the wire that led to it: its frame, its own
wiring, its lining, its lettering last, which is the order a drafting office
draws a block diagram in and signal-path order on a schematic [24], with no
subject saying how. Wires, junctions and legends belong to no part and are
drawn as the walk crosses them; a part's neighbours are taken in the order of
their items, then left to right. What the walk never reaches is drawn after
it.

**Within a cluster**, the groups with one key, the order is an open tour:

1. **Nearest first.** From wherever the pen is, it goes to the nearest group
   next, a line drawn from its nearer end, an area's lining run backwards, a
   loop started at its pixel nearest the pen; ties go to the group made first.
   Nearest neighbour comes within about a quarter of the shortest tour on
   random points, and 2-opt within about a twentieth [25]; on a plotter nearly
   all the gain is in the greedy step [26].
2. **Polished.** Then the tour is improved wherever moving a run of one to
   three groups elsewhere, turned round if they turn, or turning a stretch of
   them round, shortens the pen's journey (Or-opt, and 2-opt with vpype's
   block reverse [27]), for at most three rounds and only in clusters of at
   most 600 groups. The point is the look more than the time: it removes the
   greedy order's tail of stragglers, the pen darting back across the sheet
   for strokes it left behind [26].
3. **Seams.** Each loop is started where the pen comes to it and goes on from
   it soonest, and goes round the way the pen was heading as it came to it, as
   a hand carries a stroke on rather than doubling back.

A space-filling order for line work was rejected: it carries the pen two to
four times as far as the greedy order.

### Motion

The pen's work is a list of moves: carried up from one stroke to the next,
drawing along a stroke, touching down for a dot, and still while it settles or
pauses. Each move is planned on the stroke's path simplified to its corners
(Douglas–Peucker at 0.75 px [27], so a pixel line's stair steps are no
corners), as blocks of constant acceleration [28]:

- each corner is turned no faster than cutting it by the junction deviation δ
  allows, `v=sqrt(a·δ·s/(1−s))` where `s=sqrt((1+u·w)/2)` for the unit
  directions `u` and `w` either side of it (GRBL's cornering [6], as the
  AxiDraw and saxi plan [4][5]), and the stroke's ends are at rest;
- a pass forwards and a pass backwards limit each corner's speed to what can
  be reached from the one before and stopped from by the one after, and each
  segment is a trapezoid of speed, or a triangle if it is too short to reach
  the cruise;
- a segment shorter than 32 px is stepped at a far higher acceleration than a
  long one is ramped at, so lettering, hatching and ticks peck while long
  edges ease, Calcomp's dual mode [8];
- the pen draws at a speed for its weight, as HP-GL and AutoCAD let each pen
  have its own [15][29]: light construction fastest, the ink pen slowest;
- carried up, it moves faster than it draws, as the AxiDraw and saxi do
  [4][5]; it lifts with no delay, as HP's voice-coil pen does [3], and after a
  move longer than 48 px it takes a frame to settle, as the 7550A drops slowly
  after a long move [3]; after a shorter one it is already down, HP's
  dual-level lift [9].

The hand's figures, for the laptop's sheet:

| | Value |
|---|---|
| Drawing speed, faint / line / ink pens | 2,600 / 2,000 / 1,600 px/s |
| Acceleration along a long line | 16,000 px/s² |
| Acceleration along a line under 32 px | 300,000 px/s² |
| Junction deviation | 3 px: a right angle keeps a fifth of the speed, a 30° turn three quarters, a circle's chords all of it |
| Carried up | 4,000 px/s at 40,000 px/s² |
| Settling after a move over 48 px | a frame |
| Between stages, and between parts | 150 ms, 60 ms |
| A dot | 1/45 s |

These are not a real machine's numbers: a real plotter squeezed into seconds
would ramp and drop in a few milliseconds, all shorter than a frame. What
carries over from real machines is their ratios: travel faster than drawing,
ramps tens of pixels long, a right angle keeping a tenth to a third of the
speed and a circle all of it, a frame's drop after a long dart and none after
a short one [3][30][4].

### Pacing

A plot plays the hand's moves 2.25 times as fast, its pauses as they are: a
time-lapse of the draughtsman's work. A sheet's plot takes as long as that
does, with a tenth of a second after it with the pen gone, so the last frames
show the drawing as the run begins. So the pen moves at the same pace on every
sheet, and a dense sheet plots for longer than a sparse one. On screen the ink
pen cruises at 120 px a frame and reaches it in under a frame and a half over
80 px, a 300 px dart takes a little over two frames, and lettering runs at
about 70 to 110 letters a second.

The plot is never shorter than 5 s nor longer than 15 s: a sparse drawing is
not over in a moment, and a dense one does not drag. Past 15 s the moves are
played faster to fit, and under 5 s slower; the pauses keep their time unless
they would take more than half the plot. The pace and the range come from
`quadrille-screensaver plot-stats` over every subject, on both of the desk's
outputs and on every made-up machine: at 2.25 the sheets' work takes from 5.3
s (the Cooke triplet on the ultrawide) to 17.9 s (the gears on the ultrawide,
with their section), and 28 of the 32 different sheets plot at the hand's
pace. The four past 15 s, the ultrawide gears and the cooling of the made-up
desktop and server, are hurried by 26% at most. One length for every plot
would have played the same sheets from 0.9 to 4.5 times as fast as the hand.

| Sheet | Laptop | Ultrawide |
|---|---|---|
| Gears | 9.1 s | 15 s (17.9 s at the pace) |
| Four-stroke | 11.9 s | 8.9 s |
| Cooke triplet | 6.2 s | 5.3 s |
| Topology of the made-up laptop | 12.5 s | 12.4 s |
| Cooling of the made-up laptop | 11.2 s | 13.8 s |

The schedule knows each sheet's length before the sheet starts: when it puts a
sheet in, it asks how long the plot of that subject takes on that output,
worked out from the drawing as it stands then, and keeps the answer, so the
schedule is a function of its seed and the time. The sheet's plot is then
fitted to that length. On a live machine whose readings have moved on by the
time the sheet starts, the pace is off by as little as the drawing changed.

### How the pen is shown

Everything is a virtual pixel wide, in the palette's colours, with no
blending. The accent is the palette's colour for the machine at work.

| | |
|---|---|
| The head, down | A crosshair, arms of 3 with a gap of 1, round the pixel just drawn, which shows through |
| The head, up | An open crosshair, arms of 2 with a gap of 3: hovering |
| Settling | The head down with its centre lit, for the frame it settles |
| On every frame | The pen is on every frame of the plot: carried, hovering, drawing or going home |
| The carriage | A 3 px tick in the sheet's zone bands above and below the head, its carriage riding the rails, read off the sheet's rulers. Ticks at the sides as well would add two places to repaint every frame and take the plot past its share of the frame. |
| Wet ink | What the pen inked in the last 50 ms is in the accent until it dries to its tone: a tail longer the faster the pen goes, short in a corner and in lettering, so it shows the pen's speed |
| The compass | While the pen draws a circle or an arc of a radius of 12 px or more, a faint hairline from its centre to the head and the compass point lit at the centre: draughtsmen ink their circles with a compass first [13], and it shows why the pen goes round |
| Home | The corner of the zone band at the foot on the left, off the drawing, where a plotter's origin is [15]. The pen starts there, goes back there when it is done and waits a moment, as plotters park [31], and is gone for the plot's last tenth of a second. |

A straightedge or drafting machine was rejected as clutter over a dense
drawing, and costly to repaint; a faint pass blocked in and inked over, for
the geometry the subjects do not record; sound; and moving the paper, as a
grit-wheel plotter does, which would repaint the whole output every frame.

The renderer repaints what changes between frames, so the plot is drawn to
change little: strokes go in buckets of about 1,500 inked pixels, each kept as
a drawing of its own once plotted; the bucket in progress is a path for each
stretch of 96 pixels of a stroke, so only the stretch the pen is on is new;
and the wet ink and the compass are each a layer of their own on every frame,
empty when there is none, so the others pair with the frame before's as they
are.

### The detail view

When a part is picked out, the pen comes from home and draws the circle round
it on the view, starting where it is nearest home, goes to the circle's letter
and letters it, all in the half second the circle has. Then it goes to the
detail's window and plots the view there in the 1.6 s the view has: its
boundary circle, then the magnified marks pass by pass, nearest first, and
goes home. The circle on the view and the window come before the view, as ASME
Y14.3 and ISO 128 draw a detail [32]. The detail is as long however much it
draws, so its pauses shrink until they take at most three tenths of it.

The part may move while it is plotted, and the detail with it. So the plan is
made once, from the drawing as it stands when the part is picked out (a moment
of the sheet's time, so a frame drawn halfway through the detail plans the
same), and keeps each stroke by what it draws: a piece of a mark, the mark
known by its kind and which of the marks of that kind it is. Each frame draws
the marks as they are then, each as far along as the plan has drawn its
stroke. The circle's letter goes where the pen letters it, and once the detail
settles it is drawn whole.

### The wipe

The sheet is wiped by a line from where the pen is parked across to the right
of the sheet, easing in and out over 1.4 s, the sheet's ground painted over
what it has passed in strips that are kept once passed.

### The form

A drafting office's title block was printed and filled in by hand, so the
furniture is a form that fills in as the drawing proceeds: the frame, its
rules and what is known before the drawing is begun (title, drawing number,
domain, unit, revisions) at once; the notes as the annotation is begun; each
part's row of the parts list as its balloon's number is drawn, the one
signalling the other [33]; a view's name once its bodies are drawn; and the
scale, sheet, date and who drew it, and the front view's name, when the
drawing is done, as the draughtsman lays off the title and checks the drawing
last [13]. Each entry is a drawing of its own once typed, so filling the form
in over the whole plot repaints no more than typing it at the start would.

## How it fits the code

| Module | What |
|---|---|
| `plotter/mod.rs` | `Plot`: the strokes, the pen's work and the buckets; `Plot::at`, the wet ink, the compass, the cues the form fills in at, the plot's statistics |
| `plotter/hand.rs` | The hand's figures and its pace |
| `plotter/own.rs` | The owner buffer, and which marks touch |
| `plotter/strokes.rs` | Pieces to strokes: lines, loops, lining's diagonals, rows, glyph strokes, touches |
| `plotter/order.rs` | The stages, the axes, balloons, a diagram's growth, nearest first, the polish, seams, concentric circles, loops turned the pen's way |
| `plotter/motion.rs` | Paths to corners, junction speeds, blocks, travel and pauses, the pace and the fit |
| `plotter/pen.rs` | The head, the carriage's ticks, wet ink, the compass |
| `plotter/detail.rs` | A detail's plot, by the identity of what it draws |
| `draft/glyphs.rs` | Glyphs traced into strokes, once for each character |

The sheet (`sheet/mod.rs`) keeps the plot of the sheet showing and of the
detail showing from their first frames, draws the buckets kept and the one in
progress, the pen and its layers, and fills the form in (`sheet/plates.rs`).
The timeline (`sheet/timeline.rs`) has the plot's range of lengths, the
schedule (`sheet/schedule.rs`) asks each sheet's length as it puts it in, and
the screensaver works it out for each output from the size the sheet has on
it.

**Costs.** Working a plot out takes 0.2 to 3 ms on the sheet's first frame,
and the schedule's asking how long a subject's plot takes on an output several
milliseconds more, once for each subject and output. During the plot a frame
draws only the bucket in progress and the pen; at the 95th percentile its
repaint takes 0.5 to 0.9 ms and 8 to 13% of the laptop's output, and 0.8 to
1.3 ms and 3 to 5% of the ultrawide's, within the budget of 1 ms and 15% on
the laptop.

**Tests.** The plot's tests hold, for every sheet on both outputs: that the
last frame of the plot is the run's first, but for the traces that wait for
the run; that ink only accumulates; that the pen is on every frame of the
plot; that the plot ends on time, every stroke drawn and every bucket kept,
the pen put away and its ink dry; that a plot takes as long as its work at the
hand's pace, within its lengths, nearly every sheet of every kind of machine
at that pace and none hurried by a third; that a plot is worked out alike
every time and drawn alike by a fresh renderer; that a detail is drawn alike
from any frame of it and ends on the settled detail; that the form fills in as
the drawing proceeds; and that repainting what changed draws what a whole
frame draws, through the plot, a detail and the wipe. Below them, the strokes
ink exactly what their pieces paint, turned round or started anywhere, and the
order and motion have tests of their own.

**Not built.** Joining strokes whose ends touch into one motion, as vpype's
`linemerge` and the AxiDraw do [27][4]; covering a network of lines with Euler
trails for the fewest lifts of the pen [34]; an index of the strokes for the
nearest-first search, which on these sheets is fast enough without one.

## Sources

1. ZSA, ZSA loves pen plotting: https://blog.zsa.io/zsa-loves-pen-plotting/
2. Paul Rickards, plotter collection: https://biosrhythm.com/?p=3004
3. Hewlett-Packard Journal, April 1985 (HP 7550A: 80 cm/s, 6 g, curved-line generator, turning speed, fast and slow pen drop 34/60 ms, lift with no delay): https://docs.ampnuts.ru/eevblog.docs/HP_Agilent_Keysight/journals/1985-04.pdf
4. AxiDraw driver (defaults, motion planner, pen timing, reordering, `connect_nearby_ends`): https://github.com/evil-mad/axidraw
5. saxi planner defaults and cornering: https://raw.githubusercontent.com/nornagon/saxi/main/src/planning.ts
6. Sonny Jeon, GRBL cornering algorithm (junction deviation): https://onehossshay.wordpress.com/2011/09/24/improving_grbl_cornering_algorithm/
7. Fiorella & Mayer, drawing in front of learners, the visible hand: https://learningscientists.org/blog/2017/1/24-1
8. Calcomp patent US 4,776,097, dual-mode acceleration: https://patents.google.com/patent/US4776097A/en
9. Hewlett-Packard Journal, October 1981 (low-mass plotting, dual-level pen lift, lifts and drops a growing share of the time to plot text): https://docs.ampnuts.ru/eevblog.docs/HP_Agilent_Keysight/journals/1981-10.pdf
10. Navon (1977), Forest before trees: https://www.psytoolkit.org/library/navon.html
11. Chowdhury et al. (2022), FS-COCO: https://arxiv.org/abs/2203.02113
12. Fu, Zhou, Liu & Mitra (2011), Animated construction of line drawings: https://hongbofu.people.ust.hk/projects/animatedConstructionOfLineDrawings_SiggA11/animatedConstructionOfLineDrawings_SiggA11.pdf
13. T. E. French, *A Manual of Engineering Drawing* (1911), orders of pencilling and inking: https://archive.org/details/cu31924003643693
14. Tversky, Morrison & Bétrancourt (2002), Animation: can it facilitate?: https://hci.stanford.edu/courses/cs448b/papers/Tversky_AnimationFacilitate_IJHCS02.pdf
15. HP-GL commands (LT, LB, VS, P1 origin): https://pic.hallikainen.org/techref/language/hpgl/commands.htm ; https://www.devenezia.com/docs/HP/LJ1889.html
16. EggBot hatch fill: https://github.com/evil-mad/EggBot
17. Choset, boustrophedon cellular decomposition: https://publications.ri.cmu.edu/coverage-of-known-spaces-the-boustrophedon-cellupdar-decomposition
18. Hershey fonts: https://en.wikipedia.org/wiki/Hershey_fonts
19. J. Rose, *Mechanical Drawing Self-Taught* (1887): https://www.gutenberg.org/ebooks/23319
20. Armengaud, *The Practical Draughtsman's Book of Industrial Design* (1855), circles first, smaller first: https://digi.ub.uni-heidelberg.de/diglit/armengaud1855/0033
21. US Navy, Engineering Aid, order of inking: https://www.tpub.com/engbas/3-26.htm
22. City, University of London ME1105, AutoCAD tutorial 2a (section order): https://www.staff.city.ac.uk/~ra600/ME1105/Tutorials/CAD-1/Tutorial%20CAD-2a.htm
23. Hegarty (1992), mental animation, as summarised by Davis & Marcus: https://arxiv.org/pdf/1506.04956
24. Drafting for Electronics, schematic diagrams: https://www.gammaelectronics.xyz/drafting-for-electronics-11.html
25. Johnson & McGeoch, the TSP as a case study in local optimization: https://redmine.iam.upr.si/attachments/download/242/TSPchapter.pdf
26. Paul Butler, optimizing plots with a TSP solver (the greedy order's straggler tail): https://nb.paulbutler.org/optimizing-plots-with-tsp-solver/
27. vpype reference (`linemerge`, `linesort`, `linesimplify`, `reloop`): https://vpype.readthedocs.io/en/latest/reference.html
28. fogleman/axi (`Plan.instant`, `sort_paths`): https://github.com/fogleman/axi
29. AutoCAD Pen Optimization Level, pen sorting and per-pen speed: https://help.autodesk.com/cloudhelp/2023/ENG/AutoCAD-Core/files/GUID-025BEBB2-EBE4-4966-871D-E91B0811579E.htm
30. HP 7470A service manual (pen down 38.1 cm/s, pen up 50.8 cm/s, about 2 g): https://literature.hpcalc.org/community/hp7470a-sm-en.pdf
31. Virtual Plotter (a simulation; parks at home when done): https://github.com/killedbyapixel/virtualplotter
32. Detail-view callouts (ASME Y14.3, ISO 128): https://eng-tips.com/threads/detail-view-callout.215188
33. Mayer (2020), multimedia principles (signalling, segmenting): https://ugc.futurelearn.com/uploads/files/7d/d6/7dd6188d-c343-4311-b064-ac98d2c95abc/Multimedia_Principles._R._E._Mayer__2020.pdf
34. Inkscape Optimize Path (Euler trails, straightest continuation): https://github.com/Daekkyn/inkscapeOptimizePath
