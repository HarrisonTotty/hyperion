# HYPERION's Instrument Panel System (WIP)

Most spaceship bridge simulation games like Artemis divide their UI screens according to the ship position associated with the client. This locks players into choosing from a specific set of ship positions with little UI customization. HYPERION solves this with a dynamic, customizable docking system that redefines the traditional ship positions into presets of panel layouts. The look, feel, and behavior of these panels may be further customized through HYPERION's settings dialog.


## Instrument Panels

The following lists in detail each available instrument panel, loosely categorized by purpose.

Panels normally show only what the ship knows: what its instruments have observed and what it has charted, with each measurement's uncertainty. For testing and for a creative mode, panels can instead run in a ground-truth mode that shows everything as it really is, and says so on every panel in that mode.

### Photorealistic / Rendered

Every panel in this section except the Window and the Main Viewer shows the output of a piece of ship hardware: the external cameras, the drones, the telescope, or the mapping imager. This hardware is shared by the whole crew. Every panel showing an instrument sees the same feed, and only one crew member controls it at a time while everyone else watches what they do with it. An instrument that is damaged or unpowered says so instead of showing a picture.

Photos taken by any of these instruments are saved with the session and shared with the whole crew through the Photo Gallery.

#### Window

A photorealistic view out of the front of the ship. It is a literal window: no instrumentation, no overlays, and nothing a human eye couldn't see.

- **The eye, not a camera.** Stars show down to about magnitude 6.5, the naked eye's limit against a dark sky. The view adapts as an eye does: it adjusts to brightness within seconds but recovers in the dark slowly, so after looking at a sunlit planet or the system's star the faint stars take minutes to return (full dark adaptation takes 20–30 minutes, which HYPERION's settings may shorten). Glare from the star washes out the sky around it.
- **No zoom.** Its field of view is the window's, not a lens's.
- **A calibrated field of view.** The window only looks right when its field of view matches the angle the screen covers at the crew's eyes, 2·atan(width / (2 × distance)): a 3 m screen seen from 4 m covers 41°. HYPERION's settings ask for the screen's width and the viewing distance and compute it, so that stars and bodies appear at their true angular sizes and separations. The field of view may still be set by hand.
- **Several screens.** Several clients may each run a Window offset in heading and pitch, tiling into a wraparound view as a flight simulator's multi-screen setup does, with compensation for the bezels between monitors. An offset of 90° or 180° makes a side or rear window.

This panel is not typically associated with any particular station, and is usually allocated to a client acting solely as the window (and usually hooked up to a projector or other large screen).

#### Main Viewer

The bridge's main screen, as in Artemis or Star Trek. Where the Window always looks out, the Main Viewer shows whatever a station pushes to it:

- an External Camera's or Drone Camera's feed, or the Telescope's or Mapping Imager's latest image;
- any other panel, mirrored from the station showing it, such as the System Map during a briefing;
- an incoming communication.

The Main Viewer has no controls of its own, and nobody selects its source from it. The captain asks for something on screen by voice, and it is up to the station that has it to push its panel to the Main Viewer. A new push replaces whatever is on screen, and any station, the captain's included, may clear it. Every station can see what the Main Viewer is showing and which station pushed it. Like the Window, it usually runs on a client of its own driving a projector or large screen. A bridge may have both, or use the Main Viewer in place of the Window.

#### External Camera

A live feed from one of the cameras mounted on the ship's hull. The ship's design decides which cameras exist and where they sit; a typical ship has "front", "rear", "left", "right", "top", and "bottom"-mounted cameras, and the user switches between them.

- **Mounts.** A camera is either fixed or on a gimbal. A gimbal pans and tilts at a limited rate and within limits of travel, and the hull blocks part of what each camera can see.
- **Zoom.** Optical zoom over the lens's range, then digital zoom beyond it, which the panel marks as digital.
- **Tracking.** A gimballed camera can lock onto a body or contact and follow it.
- **Exposure.** Automatic, or manual with aperture, shutter speed, and sensitivity. Automatic exposure meters either the whole frame, a spot on the target, or the highlights, so a sunlit hull can be exposed correctly against black space, or the stars brought out at the cost of a blown-out hull.
- **Sun protection.** Pointing a camera near the star closes its shutter automatically to protect the sensor, with a warning saying why.
- **Thermal cameras.** Some mounts carry thermal-infrared cameras, which see the night side of a planet, hot spots on a hull, and engine plumes.
- **Photos.** Each photo records the time, the ship's position, the camera, and its settings along with the image, and goes to the Photo Gallery.

#### Drone Camera

The view from a camera drone launched by the ship and flown from this panel. It shows what no hull camera can: the ship itself from outside, the far side of an asteroid, or a closer look at a surface feature or a derelict.

- **Flight.** The pilot flies it directly, or sets it to hold position relative to the ship or a target, circle a target, or return and dock.
- **Limits.** It flies on its own propellant and battery, both shown, and must return to the ship to refuel and recharge. Its controls and video travel by radio, so it has a maximum range, and far enough out a noticeable delay in both (one second each way at 300,000 km). A drone that runs out of propellant or flies out of range is lost.
- **Its camera** has the same zoom, exposure, and photo controls as an External Camera.

A ship may carry more than one drone, each its own piece of shared hardware.

#### Telescope

The ship's telescope, for stars and objects too faint or too distant for the cameras. Unlike the cameras, it is a scientific instrument: its images are how the ship finds and measures what it can't otherwise see, and they show only what the telescope can actually detect with its aperture, band, and exposure.

- **Long exposures.** A longer exposure reaches fainter objects: where the sky's background light is the limit, each doubling of the exposure reaches about 0.38 magnitudes fainter. Exposures take game time and show their progress, several may be stacked into one image, and observations may be queued as a plan.
- **Pointing.** The target is chosen from the Discovery Catalog, from a map, or by coordinates. The telescope slews and settles before an exposure starts, and the ship must hold steady: firing the thrusters during an exposure smears it, so the helm has to cooperate with whoever is observing. Like real space telescopes, it refuses to point too close to the star (Hubble may not point within 50° of the Sun).
- **Bands.** Each band is its own instrument with its own optics and detector, so changing band isn't instant. Resolution depends on the band too, since a telescope resolves no finer than about 1.22 λ/D: a 1 m mirror resolves 0.14″ in visible light, but only 2.5″ in the infrared at 10 µm.
  - _Visible_, with a filter wheel for measuring colours.
  - _Ultraviolet_.
  - _Infrared_, whose detectors must be cold: tens of kelvin for the near-infrared, a few kelvin for the mid-infrared. An infrared instrument takes time and power to cool down, and the ship's own radiators glow in the infrared, so where they point matters.
  - _X-ray_, which needs grazing-incidence mirrors and photon-counting detectors, and has a small field of view.
- **Real artefacts.** Bright stars saturate and bleed and show diffraction spikes from the struts holding the secondary mirror, and cosmic rays leave bright specks on long exposures.
- **Viewing the image.** Brightness scaling (linear, logarithmic, or asinh) with a histogram; false-colour composites combining several bands; a coordinate grid; and a catalogue overlay that marks known objects and flags unmatched sources as candidate discoveries. Clicking a source measures its brightness, with error bars.

Telescope photos go to the Photo Gallery with their band, exposure, and pointing.

#### Mapping Imager

The survey camera that maps a planet or moon from orbit. It points straight down and images a strip of the surface beneath the ship as it moves along its orbit, building up a map one strip at a time. The panel shows the strip being imaged live, the body's coverage so far (what has been mapped, and at what resolution), and the ground track of the coming passes.

- **Resolution depends on altitude.** Each pixel covers the altitude times its angular size: a 10 µrad pixel at 400 km covers 4 m of ground. A lower orbit maps finer detail but in narrower strips, so a full map takes more passes.
- **Bands.** Visible light needs a lit, clear surface. Thermal infrared also works on the night side. Radar sees through cloud, as Magellan mapped Venus.
- **What it feeds.** Mapped surface fills in the Planetary Map and Local Map with surface detail.


### Navigation

Every map in this section is oriented by the galaxy's own directions, as HYPERION's displays already are: `COREWARD` and `RIMWARD` (towards and away from the galactic axis), `SPINWARD` and `ANTISPINWARD` (with and against the galaxy's rotation), and `NORTH` and `SOUTH` (along the axis). This holds at every scale, around a planet as much as across the galaxy.

The ship never knows its position exactly. It works it out from observations, as a real spacecraft does: its attitude from star trackers, and its position from the parallax of catalogued nearby stars or from pulsar timing. Every map draws the ship's position with its uncertainty and the age of its last fix. The uncertainty grows as the ship travels without a new fix, and after an FTL trip into unfamiliar space the navigator must take a new one. It also depends on what the ship can measure: close to a body the ship can range on it directly, while between the stars it relies on distant references and knows its position far less well.

#### Galactic Map

Displays the ship's appoximate position in relation to the entire galaxy. Includes markers for other cataloged discoveries.

#### Interstellar Map

Displays the ship's position relative to its local group of stars. This is essentially the 3D viewer of the "Local Chart" today.

#### System List

Lists the systems and galactic features present in the Interstellar Map (including their designation, class, number of stars, etc). Systems selected in this list are sync'd with selections on the interstellar map, like today.

#### System Map

Displays the ship's position relative to its current system - essentially the 3D viewer of the "System" view today. The map is empty when the ship is in the interstellar medium between systems.

#### System Bodies List

Displays the bodies and features present in the System Map.

#### Planetary Map

Displays the local 3D space around the planet, moon, or other large body the ship is closest to, with the body at its centre and the ship's position relative to it. This is essentially the "Focus Body" view.

#### Local Map

Displays the ship and its immediate surroundings, out to a few thousand km. When near a body's surface, this displays a scrolling topological map around the ship.

#### Route Planner

Plans a journey across the galaxy as a chain of legs from waypoint to waypoint, and draws the route on the Galactic and Interstellar Maps. A waypoint is a system, a body, or a point in space.

- **Legs.** Each leg names the drive it uses and shows its distance, travel time, energy, and arrival accuracy, as the FTL Planner works them out. A leg beyond its drive's range is flagged.
- **Stops.** The route shows where the ship must stop to refuel, recharge, or let a drive cool down, and what each stop offers, as far as the ship knows.
- **Optimising.** The planner can propose a route that minimises travel time, energy, or the number of legs, or that avoids hazards along the way.
- **Totals.** The whole route's distance, travel time, and energy, set against what the ship has.

#### FTL Planner

Plans a single faster-than-light trip with one of the ship's drives. HYPERION's ships use several semi-realistic forms of FTL, such as Alcubierre warp drives and wormhole-creating drives, and each has its own trade-offs in power consumption, accuracy, spin-up time, and more. The planner adapts to the drive selected and shows, for the chosen target:

- range, travel time, and the energy and power the trip needs;
- spin-up time, and the drive's progress while it spins up;
- the state the ship arrives in: where, and at what velocity relative to the target;
- arrival accuracy, drawn as the region the ship may arrive anywhere within, which becomes part of the ship's position uncertainty on arrival;
- the drive's own limits and hazards. An Alcubierre drive's course can't be changed once it is under way, for instance, and its bubble gathers particles in transit and releases them in a burst ahead of the ship when it stops (McMonigal, Lewis, and O'Byrne, 2012), so the arrival point must be clear;
- with more than one drive fitted, a comparison of the drives for the same trip.

The planner aims at where the target is now, not where its light shows it. A star 1,000 light-years away has moved on since that light left it (at 30 km/s, by about a tenth of a light-year), so the planner predicts the star's present position from its measured motion, and any uncertainty in that motion widens the arrival region.

#### Manoeuvre Planner

Plans the ship's sublight burns within a system, as Kerbal Space Program's manoeuvre nodes do, against a predicted trajectory drawn on the System Map and Planetary Map.

- **Burns.** Each planned burn has a start time, a direction (prograde, normal, radial, or towards a target), and a delta-v. The planner turns that into a real burn with a duration and a propellant cost from the engines' thrust, since a long burn doesn't happen in an instant.
- **The trajectory.** Predicted under the gravity of every significant body at once, rather than one body at a time as patched conics are, so it includes the perturbations that simpler model leaves out. Its uncertainty grows from the ship's position uncertainty, and is drawn with it.
- **Delta-v budget.** Each burn, and the plan as a whole, set against the delta-v the ship's propellant allows.
- **Transfer windows.** A porkchop plot for transfers to another body, charting departure date against arrival date coloured by delta-v, with the cheapest window marked. Gravity assists and aerobraking may be planned as parts of the trajectory.
- **Closest approach.** For a target, the predicted closest approach: its time, its distance, and the relative velocity there.

Planned burns are flown from the Flight Instruments.


### Scientific

#### Discovery Catalog

A detailed catalog of every named discovery (galactic features, systems, planets & bodies, lifeforms, etc), organized by type.

#### HR Diagram

A Hertzsprung–Russell diagram associated with the current local group of stars. This is essentially already built.

#### Selected Object Details

Provides a detailed overview of the selected object - be it a system selected from the Interstellar Map, a planet from the System Map, etc.


### Flight Instruments

#### Primary Flight Display

The primary display used when the ship is operating in space. The display can be switched into different "modes" which hide or show specific HUD elements and overlays.

- **Atmospheric Flight** - Traditional atmospheric flight (similar to a modern fighter jet).
- **Space Flight** - Flight in outer space.
- **NLS Flight** - Near-lightspeed (including up to lightspeed) flight in outer space.
- **FTL FLight** - Faster-than-light flight.


### Communications & Diplomacy

#### Primary Communications Panel

The main communications panel, allowing the operator to send & recieve text messages.

#### Link Status

The status of established communications links and their statistics.

#### Trade / Barter Interface

Displayed when trading or bartering has been initiatied with a space station, another ship, etc.


### Tactical & Military

(not defined until combat gameplay is designed)


### Engineering

#### Life Support

The status of life support throughout the ship.

#### Structure

The health status of the ship's hull and all of its components.

#### Power

The amount of power generated by the ships reactor, the amount of stored energy, and the allocation of power to each ship component.

#### Thermal

The heat generated by each ship component, with controls to redirect & mitigate heat buildup.

#### Propulsion

The status of each thruster along with remaining fuel.

#### Avionics & Data

The status of onboard computers and data.

#### Fault Log

A log of tripped caution and warning messages and their recovery.


### Operations & Logistics

#### Ship Designer

When docked at a space station, this allows the crew to redesign the ship.

#### Ship Inventory

A filterable list of the ship's cargo inventory.

#### Hanger

The state of every carried craft (drones now, landers and fighters later): docked, refuelling, charging, launched or lost, plus launch and recovery sequences. The Drone Camera says a ship may carry several drones, but no panel yet shows them all at once.

#### Endurance

Days left of oxygen, water, food, scrubber capacity, propellant and reactor fuel at the current rate of use.

#### Mass & Balance

Total mass, centre of mass and moments of inertia, updated as propellant drains and cargo moves. It's the spaceship version of an aircraft's weight-and-balance sheet.

#### Refuelling

Plans and monitors standard refuelling procedures and gas-giant skimming.

#### Maintenance & Repairs

Plans and displays repair and maintenance jobs.

### Miscellaneous

#### Crew Log

Essentially a Markdown-based journal for the captain or other crew members to organize notes.

#### Photo Gallery

A photo gallery for viewing photos taken by instruments that support it. Photos are saved with the session and shared by the whole crew, each with the time, position, instrument, and settings it was taken with.


### Debug / Cheating

#### Free Camera

A free-flying camera with variable speed.
