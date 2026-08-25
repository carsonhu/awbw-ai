var gamemap = document.getElementById("gamemap"),
    gamecontainer = document.getElementById("gamecontainer"),
    map = document.getElementById("map-background"),
    gamemapContainer = document.getElementById("gamemap-container"),
    replayContainer = document.getElementById("replay-container"),
    zoomInButton = document.getElementById("zoom-in"),
    zoomOutButton = document.getElementById("zoom-out"),
    menu = document.getElementById("options-menu"),
    coordsDisplay = document.getElementById("coords"),
    hpInput = document.getElementById("hp"),
    buildMenu = document.getElementById("build-menu"),
    buildMenuList = document.getElementById("units"),
    armyLogos = document.getElementById("army-logos"),
    applyPowerBtn = document.getElementById("apply-power"),
    powerIcon = document.querySelector("#apply-power img"),
    changeBuilding = document.getElementById("change-building"),
    buildingOptions = document.getElementById("building-options"),
    coPowers = document.getElementById("co-powers"),
    unwaitAllButton = document.getElementById("unwait-all"),
    unwaitBtnInf = document.querySelector("#unwait-all img"),
    findTargetBtn = document.getElementById("find-target"),
    missilesSelector = document.getElementById("planner-missiles-selector"),
    clearMissilesBtn = document.getElementById("clear-missiles"),
    plannerPlayersInfo = document.querySelector(".planner-players-info table"),
    missilesCoords = document.querySelector("#missiles-coords"),
    missilesCoordsList = document.querySelector("#missiles-coords ul"),
    lastActionsList = document.getElementById("last-actions"),
    saveStateBtn = document.getElementById("planner-save-state"),
    loadStateBtn = document.getElementById("planner-load-state"),
    loadStateInput = document.getElementById("load-state-input"),
    reloadStateBtn = document.getElementById("planner-reload-state"),
    loadError = document.getElementById("load-error"),
    baseUnits = ["Infantry", "Mech", "Recon", "APC", "Artillery", "Tank", "Anti-Air", "Missile", "Rocket", "Md.Tank", "Piperunner", "Neotank", "Mega Tank"],
    airportUnits = ["T-Copter", "B-Copter", "Fighter", "Bomber", "Stealth", "Black Bomb"],
    portUnits = ["Black Boat", "Lander", "Cruiser", "Sub", "Battleship", "Carrier"],
    lastActions = [];
var dimID;
var dimURL = window.location.href.match(/id=\d+/);

if (dimURL && dimURL[0]) {
  dimURL = dimURL[0];
  dimID = dimURL.match(/\d+/)[0];
}

var scale = localStorage.scale ? parseFloat(localStorage.scale) : 1;

if (sessionStorage[dimID + "Width"]) {
  applyCSS(gamemapContainer, {
    height: parseInt(sessionStorage[dimID + "Height"]) * scale + "px",
    width: parseInt(sessionStorage[dimID + "Width"]) * scale + "px"
  });
  applyCSS(gamemap, {
    transform: "scale(" + scale + ")",
    webkitTransform: "scale(" + scale + ")"
  });
} else {
  map.onload = function () {
    applyCSS(gamemapContainer, {
      height: map.height * scale + "px",
      width: map.width * scale + "px"
    });
    sessionStorage.setItem(dimID + "Width", map.width);
    sessionStorage.setItem(dimID + "Height", map.height);
  };
}

tidy();

function tidy() {
  var plannerName = document.getElementById("planner-name").textContent;
  var imgs = [].slice.call(document.querySelectorAll("#gamemap img")),
      map = document.getElementById("map-background"),
      symbolsRegex = /[1-9]\.gif|capture|load|anifuel|aniammo|qhp|dive/;
  var currentUnit,
      currentArmy,
      coordX,
      coordY,
      pX = 0,
      pY = 0,
      newUnitCount = 1,
      armyCounter = 1,
      selectedArmy,
      movingUnit = false; //selectedArmy is for CO powers
  //toggle for missile tiles

  var showMissiles = false; //Updated upon clicking on a building to properly register where the unit is built

  var buildingCoords = {
    x: 0,
    y: 0
  }; //For Kindle's power

  var occupiedBuildings = findOccupiedBuildings();
  var inGameTeams = findTeams();
  var missileTiles = {};
  var missileVersion = "new";
  var cursor = document.getElementById("cursor"); //This code takes the sub images of units (hp, capture, loaded, etc) and
  //puts them in the same span element as the unit's image

  imgs.forEach(function (img) {
    var imgParent = img.parentElement;

    if (imgParent.nextElementSibling) {
      var nextEl = imgParent.nextElementSibling,
          child = nextEl.firstChild;

      if (symbolsRegex.test(child.src)) {
        var id;
        currentUnit = imgParent;
        nextEl.parentElement.removeChild(nextEl);
        applyCSS(child, {
          left: function () {
            if (/capture|load|dive/.test(child.src)) {
              id = currentUnit.id + "leftIcon";
              return 0;
            } else if (/aniammo/.test(child.src)) {
              id = currentUnit.id + "leftGif";
              return 0;
            } else if (/anifuel/.test(child.src)) {
              id = currentUnit.id + "rightGif";
              return "8px";
            } else {
              id = currentUnit.id + "rightIcon";
              return "8px";
            }
          }(),
          position: "absolute",
          top: "7px"
        });
        child.setAttribute("id", id);
        imgParent.appendChild(child);
        currentUnit = null;
      }
    }
  });
  var spans = [].slice.call(document.querySelectorAll("span[id^='unit']"));
  spans.forEach(preset10HP);

  function preset10HP(span) {
    var spanChildren = [].slice.call(span.children);
    var containsImg = false;
    spanChildren.forEach(function (child) {
      if (/rightIcon$/.test(child.id)) {
        containsImg = true;
      }
    });

    if (!containsImg) {
      var img = new Image();
      img.src = "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7";
      img.id = span.id + "rightIcon";
      newUnitCount++;
      applyCSS(img, {
        display: "none",
        left: "8px",
        position: "absolute",
        top: "7px"
      });
      span.appendChild(img);
    }
  }

  appendArmies();
  var fixedImgs = [].slice.call(document.querySelectorAll("span[id^='unit'] img"));
  gamemap.onclick = showOptions;
  gamemap.onmousemove = updateCoords;
  menu.onclick = menuOptions;
  buildMenu.onclick = buildMenuOptions;
  armyLogos.onclick = armySelect;
  saveStateBtn.onclick = saveState; //The loaded state is stored as json to not overwrite the original objects when updating the planner

  var loadedStateJSON;

  loadStateInput.onchange = function () {
    var plannerState = loadStateInput.files[0];
    loadedState = loadState(plannerState);
  };

  reloadStateBtn.onclick = function () {
    if (loadedStateJSON) {
      loadedState = JSON.parse(loadedStateJSON);
      createLoadedState(loadedState);
    }
  };

  unwaitAllButton.onclick = function () {
    unwaitAll(selectedArmy);
  };

  coPowers.onchange = function () {
    var p = coPowers.value; //Change power icon if SCOP

    if (/-scop/.test(p)) {
      powerIcon.src = "terrain/aw2/bluestar.gif";
    } else {
      powerIcon.src = "terrain/aw2/redstar.gif";
    } //Change button when missile power


    resetMissileTiles();
    showMissiles = false;

    if (p === "rachel-scop" || p === "sturm-cop" || p === "sturm-scop" || p === "vb-scop") {
      changeButtons(applyPowerBtn, findTargetBtn);
    } else {
      changeButtons(findTargetBtn, applyPowerBtn);
      clearMissilesBtn.style.display = "none";
    }
  }; //Show missile tiles


  findTargetBtn.onclick = function () {
    var p = coPowers.value;
    showMissiles = true;

    if (p === "rachel-scop" || p === "sturm-cop" || p === "sturm-scop" || p === "vb-scop") {
      doMissiles(coPowers.value);
      changeButtons(findTargetBtn, applyPowerBtn);
      clearMissilesBtn.style.display = "flex";
    }
  };

  applyPowerBtn.onclick = function () {
    applyPower(selectedArmy, coPowers.value);

    if (showMissiles) {
      changeButtons(applyPowerBtn, findTargetBtn);
      clearMissilesBtn.style.display = "none";
      missilesCoords.style.display = "none";
      showMissiles = false;
    }
  };

  applyPowerBtn.onmouseover = function () {
    displayMissilesCoords("flex");
  };

  applyPowerBtn.onmouseout = function () {
    displayMissilesCoords("none");
  };

  clearMissilesBtn.onclick = clearMissiles;

  function changeButtons(invisible, visible) {
    applyCSS(invisible, {
      display: "none"
    });
    applyCSS(visible, {
      display: "flex"
    });
  }

  function displayMissilesCoords(display) {
    var p = coPowers.value;

    if (showMissiles && (p === "rachel-scop" || p === "sturm-cop" || p === "sturm-scop" || p === "vb-scop")) {
      missilesCoords.style.display = display;
    }
  } //function to change building country if a unit is on it


  function check_building_behind(unit) {
    var unitId = unit.id.match(/\d+/)[0];
    var unitX = Math.round((parseInt(unit.style.left) - xOffset) / 16, 0);
    var unitY = Math.round((parseInt(unit.style.top) - yOffset) / 16, 0); //determine unit country

    if (unit.children[0].src.indexOf("gs_") > 0) {
      var findStr = "gs_";
    } else {
      var findStr = "/";
    }

    var shortCode = unit.children[0].src.substr(unit.children[0].src.lastIndexOf(findStr) + findStr.length, 2);
    var unitsCountry = getArmyName(shortCode);
    $.post("moveplanner_building_info.php", {
      page: pageID,
      units_x: unitX,
      units_y: unitY,
      games_id: gameID
    }).done(function (response) {
      var buildingInfo = JSON.parse(response);
      var buildingID = buildingInfo.buildingID;
      var terrainType = buildingInfo.terrainType.replace(/ /g, "").toLowerCase();
      var buildingCountry = buildingInfo.buildingCountry.replace(/ /g, "").toLowerCase();
      buildingOptions.innerHTML = "";

      if (buildingID) {
        var currentBuilding = document.getElementById("building_" + buildingID).firstChild;

        var _fragment = document.createDocumentFragment();

        var buildingsToDisplay = [unitsCountry, "neutral"];

        if (terrainType != "hq" && terrainType != "silo") {
          buildingsToDisplay.forEach(function (country) {
            var buildingImg = new Image();
            var li = document.createElement("li");
            var buildingCode = armies[country] ? armies[country]["short"] : "neutral";
            buildingImg.src = "terrain/" + tPath + "/" + country + terrainType + ".gif";
            li.appendChild(buildingImg);
            li.addEventListener("click", function () {
              oldSrc = currentBuilding.src;
              currentBuilding.src = buildingImg.src;
              buildingsInfo[unitX][unitY].terrain_name = country + terrainType;
              buildingsInfo[unitX][unitY].terrain_country_code = buildingCode;
              buildingsInfo[unitX][unitY].buildings_capture = 20;

              if (currentUnit.info.units_capture === 1) {
                var captureIcon = document.getElementById("unit_" + unitId + "leftIcon");
                captureIcon.parentElement.removeChild(captureIcon);
                currentUnit.info.units_capture = 0;
                waitUnit(currentUnit.info);
              } //update building counts (towers/cities)


              if (/tower|Tower/.test(buildingsInfo[unitX][unitY].terrain_name)) {
                if (buildingCode == "neutral") {
                  //determine current city code
                  var oldCode = "";
                  Object.keys(inGameArmies).forEach(function (armyIdx) {
                    armyName = inGameArmies[armyIdx];

                    if (oldSrc.indexOf(armyName) > 0) {
                      oldCode = armies[armyName]["short"];
                    }
                  });
                  pId = findPlayerIdByCountry(oldCode);
                  playersInfo[pId]["towers"]--;
                } else {
                  var _oldCode = "";
                  Object.keys(inGameArmies).forEach(function (armyIdx) {
                    armyName = inGameArmies[armyIdx];

                    if (oldSrc.indexOf(armyName) > 0) {
                      _oldCode = armies[armyName]["short"];
                    }
                  });

                  if (_oldCode) {
                    //remove player's tower if it was preowned
                    pId = findPlayerIdByCountry(_oldCode);
                    playersInfo[pId]["towers"]--;
                  } //add newly capped tower


                  pId = findPlayerIdByCountry(buildingCode);
                  playersInfo[pId]["towers"]++;
                }
              }
            });

            _fragment.appendChild(li);
          });
          changeBuilding.style.display = "block";
          buildingOptions.appendChild(_fragment);
        }
      }
    })["catch"](function (err) {
      console.log(err);
    });
  }

  function getOffset(el) {
    var rect = el.getBoundingClientRect();
    return {
      // left: rect.left + window.scrollX,
      // top: rect.top + window.scrollY
      left: rect.left + window.pageXOffset,
      top: rect.top + window.pageYOffset
    };
  }

  function showOptions(e) {
    var regex = /unit_\d+/,
        buildRegex = /((base|airport|port)(_snow|_rain|)).gif/,
        element = e.target,
        parentElement = element.parentElement; //since the element you click on is an img, select the parent

    if (currentUnit) {
      if (movingUnit && currentUnit.info && currentUnit.info.moved !== 1) {
        waitUnit(currentUnit.info);
      }

      currentUnit = null;
      movingUnit = false;
      closeMenu(); //Remove movement tiles

      var movementTiles = [].slice.call(document.getElementsByClassName("movement-tile"));
      movementTiles.forEach(function (tile) {
        tile.parentElement.removeChild(tile);
      }); //Recalculate missile tiles after moving a unit if option is toggled

      if (showMissiles) {
        doMissiles(coPowers.value);
      }
    } else if (!calculator.selectorPosition && !currentUnit && regex.test(parentElement.id)) {
      var containerLeft = getOffset(gamecontainer).left,
          containerTop = getOffset(gamecontainer).top;
      mapLeft = getOffset(map).left, mapTop = getOffset(map).top;
      var unitId = parentElement.id.match(/\d+/)[0];
      applyCSS(menu, {
        display: "block",
        left: parseInt(parentElement.style.left) + 16 + "px",
        top: parseInt(parentElement.style.top) - 8 + "px"
      });
      currentUnit = {
        span: parentElement,
        info: unitsInfo[unitId]
      };
      displayHP(currentUnit.span);
      check_building_behind(currentUnit.span);
    } //Clicked on a building
    else if (buildRegex.test(element.src)) {
      var arr = element.src.split("/"),
          army = arr[arr.length - 1].replace(buildRegex, function (match) {
        return "";
      });

      if (army == "neutral" || army == "gs_neutral") {
        return;
      } //do not show build options for neutral properties


      currentArmy = army;
      currentUnit = {
        type: "",
        span: parentElement
      };
      buildingCoords.x = coordX;
      buildingCoords.y = coordY;

      if (/base/.test(element.src)) {
        showBuildOptions(e, baseUnits);
        currentUnit.type = "base";
      } else if (/airport/.test(element.src)) {
        showBuildOptions(e, airportUnits);
        currentUnit.type = "airport";
      } else {
        showBuildOptions(e, portUnits);
        currentUnit.type = "port";
      }
    }
  }

  function menuOptions(e) {
    var unitImage = currentUnit.span.firstChild;

    if (e.target.id === "move") {
      closeMenu();
      var mType = currentUnit.info.units_movement_type;
      var mp = currentUnit.info.units_movement_points;
      var unitX = currentUnit.info.units_x;
      var unitY = currentUnit.info.units_y;
      movingUnit = true;
    } else if (e.target.id === "set-hp") {
      appendHealth();
      closeMenu();
      currentUnit = null;

      if (showMissiles) {
        doMissiles(coPowers.value);
      }
    } else if (e.target.parentElement.id === "set-icon") {
      appendIcon(e);
      closeMenu();
      currentUnit = null;
    } else if (e.target.id === "wait" && !/gs_/.test(unitImage.src)) {
      var replace = function replace(match) {
        return "gs_" + match;
      };

      if (unitImage.src.indexOf("md.tank") > 0) {
        var shortCode = unitImage.src.substr(unitImage.src.lastIndexOf("/") + 1, 2);
        var unitName = shortCode + "md.tank";
        var waitedUnit = unitImage.src.replace(unitName, "gs_" + unitName);
      } // else { var waitedUnit = unitImage.src.replace(/[a-z-]+.gif/, replace); console.log("Unit SRC = " + unitImage.src); console.log ("Waited Unit = " + waitedUnit); }
      else {
        var waitedUnit = unitImage.src.replace(/[a-z-]+.gif/, function (x) {
          return "gs_" + x;
        });
      }

      currentUnit.info.moved = 1;
      unitImage.src = waitedUnit;
      if (currentUnit.span) currentUnit = null;
      closeMenu();
    } else if (e.target.id === "unwait" && /gs_/.test(unitImage.src)) {
      var freshUnit = unitImage.src.replace(/gs_/, "");
      currentUnit.info.moved = 0;
      unitImage.src = freshUnit;
      if (currentUnit) currentUnit = null;
      closeMenu();
    } else if (e.target.id === "remove") {
      logLastAction({
        type: "Remove",
        sprite: currentUnit.span.firstChild.src,
        left: currentUnit.span.style.left,
        top: currentUnit.span.style.top,
        hp: currentUnit.info.units_hit_points
      });
      closeMenu();
      currentUnit = null;

      if (showMissiles) {
        doMissiles(coPowers.value);
      }
    } else if (e.target.id !== "hp") {
      currentUnit = null;
      closeMenu();
    }

    e.stopPropagation();
  } //Create new unit


  function buildMenuOptions(e) {
    if (e.target.tagName == "LI") {
      var unitName = e.target.textContent;
      var newUnit = createNewUnit(unitName, armies[currentArmy]["short"], currentUnit, 10, 1);
      gamemap.appendChild(newUnit);
      closeMenu(); //Recalculate missile tiles after unit build

      if (showMissiles) {
        doMissiles(coPowers.value);
      }
    }
  }

  function createNewUnit(unitName, selectedCountry, selectedBuilding, unitHP, movedState) {
    var unitSpan = document.createElement("span");
    var unitImg = new Image();
    var hpSpan = new Image();
    unitImg.src = "terrain/" + tPath + "/" + (movedState === 1 ? "gs_" : "") + selectedCountry + unitName.replace(" ", "").toLowerCase() + ".gif";
    unitSpan.id = "unit_" + newUnitCount;
    unitSpan.appendChild(unitImg);
    applyCSS(unitSpan, {
      left: selectedBuilding.span.style.left,
      position: "absolute",
      cursor: "pointer",
      top: function () {
        if (selectedBuilding.type === "base") {
          return parseInt(selectedBuilding.span.style.top) + 8 + "px";
        } else if (selectedBuilding.type === "airport") {
          return parseInt(selectedBuilding.span.style.top) + 1 + "px";
        } else {
          return parseInt(selectedBuilding.span.style.top) + 5 + "px";
        }
      }(),
      zIndex: 120
    });
    hpSpan.src = "terrain/" + tPath + "/" + unitHP + ".gif";
    hpSpan.id = "unit_" + newUnitCount + "rightIcon";
    unitSpan.appendChild(hpSpan);
    applyCSS(hpSpan, {
      display: function () {
        if (unitHP === 10) {
          return "none";
        } else {
          return "block";
        }
      }(),
      left: "8px",
      position: "absolute",
      top: "7px"
    });
    var standard_unit = genericUnits[unitName];
    var unitInfo = {
      countries_code: selectedCountry,
      generic_id: standard_unit.units_id,
      moved: movedState,
      players_id: findPlayerIdByCountry(selectedCountry),
      units_cost: standard_unit.units_cost,
      units_ammo: standard_unit.units_ammo,
      units_id: newUnitCount,
      units_hit_points: unitHP,
      units_name: unitName,
      units_x: buildingCoords.x,
      units_y: buildingCoords.y
    };
    unitsInfo[newUnitCount] = unitInfo;
    buildingsInfo[buildingCoords.x][buildingCoords.y].is_occupied = true;
    occupiedBuildings[newUnitCount] = unitInfo;
    currentUnit = null;
    currentArmy = null;
    newUnitCount++;
    return unitSpan;
  }

  function updateCoords(e) {
    var containerLeft = getOffset(gamecontainer).left,
        //container abs x,y
    containerTop = getOffset(gamecontainer).top;
    mapLeft = Math.round(getOffset(map).left, 0), //map abs x,y
    mapTop = Math.round(getOffset(map).top, 0);
    coordX = Math.floor((e.pageX - mapLeft) / (16 * scale));
    coordY = Math.floor((e.pageY - mapTop) / (16 * scale));
    var mX = (e.pageX - mapLeft) / scale,
        mY = (e.pageY - mapTop) / scale;

    if (coordX < 10) {
      var ax = "0" + coordX;
    } else {
      var ax = coordX;
    }

    if (coordY < 10) {
      var ay = "0" + coordY;
    } else {
      var ay = coordY;
    }

    if (coordX !== pX || coordY !== pY) {
      if (mX > 0 && mY > 0 && mX < map.width && mY < map.height) {
        if (movingUnit) {
          applyCSS(currentUnit.span, {
            left: coordX * 16 + "px",
            top: coordY * 16 + "px"
          }); //Check if unit is on a building

          var cursorBuilding = buildingsInfo[coordX] ? buildingsInfo[coordX][coordY] : null;
          var unitOnBuilding = occupiedBuildings[currentUnit.info.units_id];
          var unitsName = currentUnit.info.units_name;

          if (cursorBuilding && !unitOnBuilding) {
            occupiedBuildings[currentUnit.info.units_id] = currentUnit.info;
            cursorBuilding.is_occupied = true; //Set building HP for Rachel's missiles calculation

            if ((unitsName === "Infantry" || unitsName === "Mech") && currentUnit.info.units_capture === 1 && currentUnit.info.countries_code !== cursorBuilding.terrain_country_code) {
              cursorBuilding.buildings_capture -= currentUnit.info.units_hit_points;
            }
          } else if (!cursorBuilding && unitOnBuilding) {
            delete occupiedBuildings[currentUnit.info.units_id];
            var unitX = unitOnBuilding.units_x;
            var unitY = unitOnBuilding.units_y;
            buildingsInfo[unitX][unitY].is_occupied = false; //Reset building HP

            if (unitOnBuilding.units_name === "Infantry" || unitOnBuilding.units_name === "Mech") {
              buildingsInfo[unitX][unitY].buildings_capture = 20;
            }
          }

          currentUnit.info.units_x = coordX;
          currentUnit.info.units_y = coordY;
        }

        applyCSS(cursor, {
          display: "block",
          left: coordX * 16 - 5 + "px",
          top: coordY * 16 - 5 + "px"
        });
        var coords = "(" + ax + "," + ay + ")";
        coordsDisplay.textContent = coords;
        pX = coordX;
        pY = coordY;
      }
    }
  }

  function displayHP(currentUnit) {
    var hpRegex = /[1-9]{1}\.gif/;
    var childIdx = currentUnit.childElementCount - 1;
    var unitHP = 0;

    if (childIdx == 1) {
      if (/[1-9]\.gif/.test(currentUnit.children[1].src)) {
        unitHP = parseInt(currentUnit.children[1].src.match(/[1-9]\.gif/));
      } else {
        unitHP = 10;
      }
    } else if (childIdx == 2) {
      if (/[1-9]\.gif/.test(currentUnit.children[1].src)) {
        unitHP = parseInt(currentUnit.children[1].src.match(/[1-9]\.gif/));
      } else if (/[1-9]\.gif/.test(currentUnit.children[2].src)) {
        unitHP = parseInt(currentUnit.children[1].src.match(/[1-9]\.gif/));
      } else {
        unitHP = 10;
      }
    }

    hpInput.value = unitHP;
  }

  function appendHealth() {
    var hp = document.getElementById("unit_" + currentUnit.info.units_id + "rightIcon");

    if (hpInput.value <= 10 && hpInput.value >= 1) {
      hpInput.value = Math.round(hpInput.value);
      var newHP = "terrain/" + tPath + "/" + hpInput.value + ".gif";

      if (/10./.test(hp.src)) {
        applyCSS(hp, {
          display: "block"
        });
      } else if (hpInput.value == "10") {
        applyCSS(hp, {
          display: "none"
        });
      } else {
        applyCSS(hp, {
          display: "block"
        });
      }

      hp.src = newHP;
      currentUnit.info.units_hit_points = parseInt(hpInput.value);
    }
  } //Append left side icons(capture, load, dive) to the unit's span


  function appendIcon(e) {
    var unitId = currentUnit.info.units_id;
    var unitsName = currentUnit.info.units_name;
    var unitX = currentUnit.info.units_x;
    var unitY = currentUnit.info.units_y;
    var leftIcon = document.getElementById("unit_" + unitId + "leftIcon"),
        newSrc = "terrain/" + tPath + "/" + e.target.id + ".gif";

    if (e.target.id === "remove" && leftIcon) {
      leftIcon.parentElement.removeChild(leftIcon);

      if (unitsName === "Infantry" || unitsName === "Mech") {
        currentUnit.info.units_capture = 0;

        if (showMissiles) {
          doMissiles(coPowers.value);
        }
      }
    }

    if (e.target.id === "capture" || e.target.id === "load" || e.target.id === "subdive2") {
      if (leftIcon) {
        leftIcon.src = newSrc;
      } else {
        var img = new Image();
        img.src = newSrc;
        img.id = "unit_" + unitId + "leftIcon";
        applyCSS(img, {
          left: 0,
          position: "absolute",
          top: "7px"
        });
        currentUnit.span.appendChild(img);
      }

      if (currentUnit.info.moved !== 1) {
        waitUnit(currentUnit.info);
      }
    }

    if (e.target.id === "capture" && (unitsName === "Infantry" || unitsName === "Mech")) {
      currentUnit.info.units_capture = 1;

      if (buildingsInfo[unitX] && buildingsInfo[unitX][unitY] && currentUnit.info.countries_code !== buildingsInfo[unitX][unitY].terrain_country_code) {
        buildingsInfo[unitX][unitY].buildings_capture -= currentUnit.info.units_hit_points;

        if (showMissiles) {
          doMissiles(coPowers.value);
        }
      }
    }
  } //Display proper units upon clicking on a building(base, airport, port)


  function showBuildOptions(e, options) {
    buildMenuList.innerHTML = "";
    var building = e.target.parentElement;
    fragment = document.createDocumentFragment();
    var containerLeft = getOffset(gamecontainer).left,
        containerTop = getOffset(gamecontainer).top;
    mapLeft = getOffset(map).left, mapTop = getOffset(map).top;
    thisTop = parseInt(building.style.top);
    applyCSS(buildMenu, {
      display: "block",
      left: parseInt(building.style.left) + 16 + "px",
      top: thisTop + "px"
    });
    options.forEach(function (unit) {
      var li = document.createElement("li");
      li.textContent = unit;
      fragment.appendChild(li);
    });
    buildMenuList.appendChild(fragment);
  } //This is to add the logos of in-game countries available for cop/scop selection


  function appendArmies() {
    var fragment = document.createDocumentFragment(),
        sortedArmies = inGameArmies.sort(sortArmies);
    var armyCount = 1;
    sortedArmies.forEach(function (army) {
      var armyLi = document.createElement("li"),
          img = new Image();
      img.setAttribute("src", "terrain/" + tPath + "/" + armies[army]["short"] + "logo.gif");

      if (armyCount == 1) {
        armyLi.classList.add("green-border");
        selectedArmy = armies[army]["short"];
      }

      img.setAttribute("id", armies[army]["short"] + "-logo");
      armyLi.appendChild(img);
      fragment.appendChild(armyLi);
      unwaitBtnInf.src = "terrain/aw2/" + selectedArmy + "infantry.gif";
      armyCount++;
    });
    armyLogos.innerHTML = "";
    armyLogos.appendChild(fragment);
  } //This is to select the army to apply COP/SCOP on


  function armySelect(e) {
    var target = e.target,
        logosArr = [].slice.call(document.querySelectorAll("#army-logos li"));
    logosArr.forEach(resetBorder);

    if (/^\D{2}-logo$/.test(target.id)) {
      target.closest("li").classList.add("green-border");
      selectedArmy = target.id.replace("-logo", "");
      resetMissileTiles();
      var p = coPowers.value;

      if (p === "rachel-scop" || p === "sturm-cop" || p === "sturm-scop" || p === "vb-scop") {
        changeButtons(applyPowerBtn, findTargetBtn);
      } else {
        changeButtons(findTargetBtn, applyPowerBtn);
      }
    }

    unwaitBtnInf.src = "terrain/aw2/" + selectedArmy + "infantry.gif";

    function resetBorder(logo) {
      if (logo.classList.contains("green-border")) {
        logo.classList.remove("green-border");
      }
    }
  }

  function applyPower(selectedCountry, power) {
    //Only loop through units on building if Kindle's COP
    if (power === "kindle-cop") {
      var occupiedBuildings = findOccupiedBuildings();

      for (var unit in occupiedBuildings) {
        changeHP(occupiedBuildings[unit]);
      }
    } else if (power === "sensei-cop") {
      addSenseiUnits("Infantry");
    } else if (power === "sensei-scop") {
      addSenseiUnits("Mech");
    } else if (showMissiles && (power === "rachel-scop" || power === "sturm-cop" || power === "sturm-scop" || power === "vb-scop")) {
      for (var _unit in unitsInfo) {
        var unitX = unitsInfo[_unit].units_x;
        var unitY = unitsInfo[_unit].units_y;

        if (missileTiles[unitX] && missileTiles[unitX][unitY]) {
          var missileCount = missileTiles[unitX][unitY]; //Since more than one missile can land on a tile

          while (missileCount > 0) {
            changeHP(unitsInfo[_unit]);
            missileCount--;
          }
        }
      }

      resetMissileTiles();
    } else {
      for (var _unit2 in unitsInfo) {
        changeHP(unitsInfo[_unit2]);
      }
    }

    function changeHP(unit) {
      if (selectedCountry) {
        if (unit.units_carried == "Y") {} else {
          var unitsCountryCode = unit.countries_code;
          var hpSpan = document.querySelector("#unit_" + unit.units_id + "rightIcon");

          if (power === "andy-cop" && unitsCountryCode === selectedCountry) {
            addHP(2, unit, hpSpan);
          } else if (power === "andy-scop" && unitsCountryCode === selectedCountry) {
            addHP(5, unit, hpSpan);
          } else if (power === "hawke-cop") {
            if (unitsCountryCode === selectedCountry) {
              addHP(1, unit, hpSpan);
            } else if (inGameTeams[selectedCountry] !== inGameTeams[unitsCountryCode] || unitsCountryCode !== selectedCountry) {
              removeHP(1, unit, hpSpan);
            }
          } else if (power === "hawke-scop") {
            if (unitsCountryCode === selectedCountry) {
              addHP(2, unit, hpSpan);
            } else if (inGameTeams[selectedCountry] !== inGameTeams[unitsCountryCode] || unitsCountryCode !== selectedCountry) {
              removeHP(2, unit, hpSpan);
            }
          } else if (power === "drake-cop") {
            if (inGameTeams[selectedCountry] !== inGameTeams[unitsCountryCode] || unitsCountryCode !== selectedCountry) {
              removeHP(1, unit, hpSpan);
            }
          } else if (power === "drake-scop") {
            if (inGameTeams[selectedCountry] !== inGameTeams[unitsCountryCode] || unitsCountryCode !== selectedCountry) {
              removeHP(2, unit, hpSpan);
            }
          } else if (power === "kindle-cop") {
            if (inGameTeams[selectedCountry] !== inGameTeams[unitsCountryCode] || unitsCountryCode !== selectedCountry) {
              removeHP(3, unit, hpSpan);
            }
          } else if (power === "rachel-scop" && showMissiles) {
            removeHP(3, unit, hpSpan);
          } else if (power === "sturm-cop" && showMissiles) {
            removeHP(4, unit, hpSpan);
          } else if (power === "sturm-scop" && showMissiles) {
            removeHP(8, unit, hpSpan);
          } else if (power === "vb-scop" && showMissiles) {
            if (missileVersion === "v3" || inGameTeams[selectedCountry] !== inGameTeams[unitsCountryCode] || unitsCountryCode !== selectedCountry) {
              removeHP(3, unit, hpSpan);
              waitUnit(unit);
            }
          }
        }
      }
    }

    function addHP(hpAmount, unit, hpSpan) {
      var newHP = unit.units_hit_points + hpAmount;

      if (newHP >= 10) {
        applyCSS(hpSpan, {
          display: "none"
        });
        newHP = 10;
      }

      unit.units_hit_points = newHP;
      hpSpan.src = "terrain/aw2/" + newHP + ".gif";
    }

    function removeHP(hpAmount, unit, hpSpan) {
      var unitHP = unit.units_hit_points;
      var newHP = unitHP - hpAmount;

      if (unitHP === 10 && newHP < 10) {
        applyCSS(hpSpan, {
          display: "block"
        });
      } else if (newHP < 1) {
        newHP = 1;
      }

      unit.units_hit_points = newHP;
      hpSpan.src = "terrain/aw2/" + newHP + ".gif";
    }

    function addSenseiUnits(unitName) {
      var fragment = document.createDocumentFragment();

      for (var x in buildingsInfo) {
        for (var y in buildingsInfo[x]) {
          var building = buildingsInfo[x][y];
          var buildingCountry = building.terrain_country_code ? building.terrain_country_code : building.countries_code;

          if (building && buildingCountry === selectedCountry && !building.is_occupied && /city|City/.test(building.terrain_name)) {
            var bId = building.buildings_id ? building.buildings_id : building.id;
            var buildingSpan = document.getElementById("building_" + bId);
            var selectedBuilding = {
              span: buildingSpan,
              type: "city"
            };
            buildingCoords.x = building.buildings_x || building.buildings_x === 0 ? building.buildings_x : building.x;
            buildingCoords.y = building.buildings_y || building.buildings_y === 0 ? building.buildings_y : building.y;
            var newUnit = createNewUnit(unitName, buildingCountry, selectedBuilding, 9, 0);
            fragment.appendChild(newUnit);
          }
        }
      }

      gamemap.appendChild(fragment);
    }
  }

  function logLastAction(action) {
    lastActions.push(action);
    var li = document.createElement("li"),
        loggedUnit = currentUnit.span,
        loggedUnitInfo = currentUnit.info;
    applyCSS(currentUnit.span, {
      left: "",
      position: "relative",
      top: ""
    });
    li.appendChild(currentUnit.span);
    var currentUnitId = currentUnit.info.units_id;
    delete unitsInfo[currentUnitId];
    var mapLeft = getOffset(map).left,
        mapTop = getOffset(map).top;

    if (action.left || action.top) {
      var x = Math.floor(parseInt(action.left) / 16),
          y = Math.floor(parseInt(action.top) / 16),
          coords = "(" + x + "," + y + ")";
      li.innerHTML += coords;
    }

    li.onclick = reverseAction;
    lastActionsList.appendChild(li);

    function reverseAction() {
      applyCSS(loggedUnit, {
        left: action.left,
        position: "absolute",
        top: action.top
      });
      unitsInfo[currentUnitId] = loggedUnitInfo;
      gamemap.appendChild(loggedUnit);
      li.parentElement.removeChild(li);

      if (showMissiles) {
        doMissiles(coPowers.value);
      }
    }
  }

  function doMissiles(power) {
    var missiles = [];
    var colors = [{
      background: "255, 80, 80, 0.4",
      border: "#D01919"
    }, {
      background: "63, 63, 191, 0.4",
      border: "#3030C9"
    }, {
      background: "21, 243, 58, 0.4",
      border: "#009919"
    }];

    if (power === "rachel-scop") {
      missiles.push("INF", "COST", "HP");
    } else if (power === "sturm-cop" || power === "sturm-scop") {
      missiles.push("COST_STURM");
    } else if (power === "vb-scop") {
      missiles.push("COST_VB");
    } //Reset current missiles data


    resetMissileTiles();
    missilesCoordsList.innerHTML = "";
    missiles.forEach(function (type, i) {
      var options = {
        gamesId: gamesId,
        ownersCountry: selectedArmy,
        plannerState: {
          buildingsInfo: buildingsInfo,
          playersInfo: playersInfo,
          unitsInfo: unitsInfo
        },
        type: type
      }; // console.log(options);

      axios.post("api/moveplanner/planner_find_target.php", options).then(function (res) {
        var data = res.data;
        var x = parseInt(data["x"]);
        var y = parseInt(data["y"]);
        missileVersion = data["version"] || "new";
        drawMissileTiles(x, y, colors[i]);

        if (type == "COST_VB" || type == "COST_STURM") {
          type = "COST";
        }

        showMissileCoords(type, x, y);
      })["catch"](function (err) {
        console.log(err);
      });
    });
  }

  function drawMissileTiles(x, y, color) {
    var fragment = document.createDocumentFragment();
    var borders = {
      "-2": {
        0: "2px 0 2px 2px"
      },
      "-1": {
        "-1": "2px 0 0 2px",
        1: "0 0 2px 2px"
      },
      0: {
        "-2": "2px 2px 0 2px",
        2: "0 2px 2px 2px"
      },
      1: {
        "-1": "2px 2px 0 0",
        1: "0 2px 2px 0"
      },
      2: {
        0: "2px 2px 2px 0"
      }
    };

    var _loop = function _loop(i) {
      var _loop2 = function _loop2(j) {
        if (Math.abs(i) + Math.abs(j) <= 2 && x + i >= 0 && y + j >= 0 && x + i < map.width / 16 && y + j < map.height / 16) {
          var span = document.createElement("span");
          span.className = "missile-tile";
          applyCSS(span, {
            background: "rgba(" + color.background + ")",
            borderColor: color.border,
            borderStyle: function () {
              if (Math.abs(i) + Math.abs(j) === 2) {
                return "solid";
              }
            }(),
            borderWidth: findBorder(i, j),
            boxSizing: "border-box",
            height: 16 + "px",
            left: 16 * (x + i) + "px",
            position: "absolute",
            top: 16 * (y + j) + "px",
            width: 16 + "px",
            zIndex: 119
          });
          fragment.appendChild(span);

          if (!missileTiles[x + i]) {
            missileTiles[x + i] = {};
          }

          if (!missileTiles[x + i][y + j]) {
            missileTiles[x + i][y + j] = 0;
          }

          missileTiles[x + i][y + j] += 1;
        }
      };

      for (var j = -2; j <= 2; j++) {
        _loop2(j);
      }
    };

    for (var i = -2; i <= 2; i++) {
      _loop(i);
    }

    gamemap.appendChild(fragment);

    function findBorder(i, j) {
      if (Math.abs(i) + Math.abs(j) === 2) {
        return borders[i][j];
      }
    }
  }

  function resetMissileTiles() {
    var drawnTiles = [].slice.call(document.getElementsByClassName("missile-tile"));

    if (drawnTiles.length != 0) {
      drawnTiles.forEach(function (tile) {
        tile.parentElement.removeChild(tile);
      });
      missileTiles = {};
      missileVersion = "new";
    }
  }

  function clearMissiles() {
    var missileTilesEls = [].slice.call(document.getElementsByClassName("missile-tile"));
    missileTilesEls.forEach(function (tile) {
      tile.parentElement.removeChild(tile);
    });
    changeButtons(applyPowerBtn, findTargetBtn);
    clearMissilesBtn.style.display = "none";
    missileTiles = {};
    missileVersion = "new";
    showMissiles = false;
  }

  function showMissileCoords(type, x, y) {
    var li = document.createElement("li");
    var coords = x + "," + y;
    li.textContent = type + ": " + coords;
    missilesCoordsList.appendChild(li);
  }

  function waitUnit(unit) {
    var unitImg = document.getElementById("unit_" + unit.units_id).firstChild;
    var waitedUnit = unitImg.src.replace(/["a-z-._"]+.gif/, function (match) {
      if (!match.match("gs_")) {
        return "gs_" + match;
      } else {
        return match;
      }
    });
    unit.moved = 1;
    unitImg.src = waitedUnit;
  }

  function saveState() {
    var plannerState = {
      buildingsState: buildingsInfo,
      fogArray: fogArray,
      plannerName: plannerName,
      unitsState: unitsInfo
    };
    var url = window.URL.createObjectURL(new Blob([JSON.stringify(plannerState)], {
      type: "application/json"
    }));
    var a = document.createElement("a");
    a.href = url;
    a.download = plannerName + "_moveplanner.json";
    document.body.appendChild(a);
    a.click();
  }

  function loadState(state) {
    var formData = new FormData();
    formData.append("plannerState", state);
    formData.append("plannerName", plannerName);
    axios.post("api/moveplanner/planner_load_state.php", formData).then(function (res) {
      var state = res.data; //error message

      if (typeof state == "string") {
        loadError.textContent = state;
      } else {
        //The state is only passed as a reference so store as JSON
        loadedStateJSON = JSON.stringify(state);
        createLoadedState(state);
        applyCSS(loadStateBtn, {
          width: "50%"
        });
        applyCSS(reloadStateBtn, {
          display: "flex",
          width: "50%"
        });
      }
    })["catch"](function (err) {
      loadError.textContent = err;
    });
  }

  function createLoadedState(state) {
    //if(state.fogArray) {
    //map.src = "data:image/png;base64," + state.fogImage;
    //}
    fogArray = state.fogArray;
    drawFogCanvas(fogArray);
    createLoadedUnits(state.unitsState, state.terrainPath);
    createLoadedBuildings(state.buildingsState, state.terrainPath);
    loadError.textContent = "";
  }

  function createLoadedUnits(state, terrainPath) {
    //Wipe the current board
    var currentUnits = [].slice.call(document.querySelectorAll("span[id^='unit_']"));
    currentUnits.forEach(function (unit) {
      unit.parentElement.removeChild(unit);
    });
    unitsInfo = {};
    var fragment = document.createDocumentFragment(); //loop units and create them

    var _loop3 = function _loop3(_unit3) {
      _unit3 = state[_unit3];
      var unitSpan = document.createElement("span");
      var unitImg = new Image();
      var unitHP = new Image();
      unitSpan.id = "unit_" + _unit3.units_id;
      unitImg.src = terrainPath + (_unit3.moved === 1 ? "gs_" : "") + _unit3.countries_code + _unit3.units_name.toLowerCase().replace(' ', '') + ".gif";
      unitHP.id = "unit_" + _unit3.units_id + "rightIcon";

      if (!_unit3.units_hit_points || _unit3.units_hit_points == 10) {
        _unit3.units_hit_points = 10;
        unitHP.src = "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7";
      } else {
        unitHP.src = "terrain/aw2/" + _unit3.units_hit_points + ".gif";
      }

      applyCSS(unitSpan, {
        cursor: "pointer",
        left: _unit3.units_x * 16 + "px",
        position: "absolute",
        top: _unit3.units_y * 16 + "px",
        zIndex: 120
      });
      applyCSS(unitHP, {
        display: function () {
          if (_unit3.units_hit_points === 10) {
            unit = _unit3;
            return "none";
          }
        }(),
        left: "8px",
        position: "absolute",
        top: "7px"
      });
      unitSpan.appendChild(unitImg);
      unitSpan.appendChild(unitHP);

      if (_unit3.units_capture === 1) {
        var captureIcon = new Image();
        captureIcon.id = "unit_" + _unit3.units_id + "leftIcon";
        captureIcon.src = "terrain/capture.gif";
        applyCSS(captureIcon, {
          left: 0,
          position: "absolute",
          top: "7px"
        });
        unitSpan.appendChild(captureIcon);
      }

      fragment.appendChild(unitSpan);
      unitsInfo[_unit3.units_id] = _unit3;
      unit = _unit3;
    };

    for (var unit in state) {
      _loop3(unit);
    }

    gamemap.appendChild(fragment);
  }

  function createLoadedBuildings(buildingsState, terrainPath) {
    buildingsInfo = {};

    for (var x in buildingsState) {
      buildingsInfo[x] = {};

      for (var y in buildingsState[x]) {
        var building = buildingsState[x][y]; //Different naming for replay/game

        var buildingId = building.id ? building.id : building.buildings_id;
        var buildingName = building.terrain_name.replace(/ /g, "").toLowerCase();
        var buildingImg = document.querySelector("#building_" + buildingId + " img");
        buildingImg.src = terrainPath + buildingName + ".gif";
        buildingsInfo[x][y] = building;
      }
    }
  }
}

window.onload = function () {
  applyCSS(gamemapContainer, {
    height: map.height * scale + "px",
    width: map.width * scale + "px"
  });
  var initialScale = scaleAdd(0);
  applyCSS(gamemapContainer, {
    visibility: "visible"
  });

  if (autoScroll) {
    if (sessionStorage.getItem("pageX" + gameId) || sessionStorage.getItem("pageY" + gameId)) {
      var x = sessionStorage.getItem("pageX" + gameId),
          y = sessionStorage.getItem("pageY" + gameId);
      window.setTimeout(function () {
        window.scrollTo(x, y);
      }, 0);
    }

    document.addEventListener("scroll", storePos);
  }
}; //store the last position


function storePos() {
  var pageY = $(document).scrollTop();
  var pageX = $(document).scrollLeft();
  sessionStorage.setItem("pageY" + gameId, pageY);
  sessionStorage.setItem("pageX" + gameId, pageX);
}

var new_width, scaled_height;

function scaleAdd(n) {
  if (scale < 0.5 && n < 0) {
    return;
  } //do not allow to get too small
  else if (scale > 2.9 && n > 0) {
    return;
  } //do not allow to get too big
  else {
    scale = Math.round((scale + n) * 10) / 10; //increase scale

    new_width = parseInt(map.width) * scale; //make scrollable if too big

    if (new_width > 1190) {
      scaled_height = 1190 * (map.height / map.width);
      applyCSS(gamemapContainer, {
        height: scaled_height + "px",
        width: "1190px"
      });
      gamemapContainer.style.overflowY = "visible";
      gamemapContainer.style.overflowX = "scroll";
      gamemap.style.transform = "scale(" + scale + ")";
      gamemap.style.webkitTransform = "scale(" + scale + ")";
    } //otherwise scale
    else {
      applyCSS(gamemapContainer, {
        height: map.height * scale + "px",
        width: map.width * scale + "px",
        overflow: "visible"
      });
      gamemap.style.transform = "scale(" + scale + ")";
      gamemap.style.webkitTransform = "scale(" + scale + ")";
    }

    localStorage.setItem("scale", scale);

    if (parseInt(gamemapContainer.style.width) > 990) {
      applyCSS(replayContainer, {
        alignItems: "center",
        display: "flex",
        flexDirection: "column"
      });
    } else {
      applyCSS(replayContainer, {
        alignItems: "start",
        flexDirection: "row"
      });
    }
  }
}

zoomInButton.addEventListener("click", function () {
  scaleAdd(0.1);
});
zoomOutButton.addEventListener("click", function () {
  scaleAdd(-0.1);
});

function closeMenu() {
  changeBuilding.style.display = "none";
  buildingOptions.innerHTML = "";
  menu.style.display = "none";
  buildMenu.style.display = "none";
  return "Closed";
}

function sortArmies(a, b) {
  return armies[a].order - armies[b].order;
}

function getArmyName(shortName) {
  var keys = Object.keys(armies);

  for (var x = 0; x < keys.length; x++) {
    key = keys[x];

    if (armies[key]["short"] == shortName) {
      return key;
    }
  }
}

function findPlayerIdByCountry(country) {
  for (var player in playersInfo) {
    if (playersInfo[player].countries_code === country) {
      return playersInfo[player].players_id;
    }
  }
}

function findTeams() {
  var teams = {};

  for (var player in playersInfo) {
    var countryCode = playersInfo[player].countries_code;
    var team = playersInfo[player].players_team;
    teams[countryCode] = team;
  }

  return teams;
} //List units that are on a building


function findOccupiedBuildings() {
  var occupiedBuildings = {};

  for (var _unit4 in unitsInfo) {
    var unitX = unitsInfo[_unit4].units_x;
    var unitY = unitsInfo[_unit4].units_y; // if(buildingsInfo[unitX] && buildingsInfo[unitX][unitY]) { console.log("Building @ [" + unitX + "," + unitY + "]: " + buildingsInfo[unitX][unitY].terrain_name + " | Result --> " + buildingsInfo[unitX][unitY].terrain_name.indexOf("Missile Silo")); }

    if (buildingsInfo[unitX] && buildingsInfo[unitX][unitY] && buildingsInfo[unitX][unitY].terrain_name.indexOf("Missile Silo") < 0) {
      //exclude (empty) missile silos
      occupiedBuildings[unitsInfo[_unit4].units_id] = unitsInfo[_unit4];
    }
  }

  return occupiedBuildings;
}

function unwaitAll(selectedCountry) {
  for (var _unit5 in unitsInfo) {
    var unitsCountry = unitsInfo[_unit5].countries_code;

    if (unitsInfo[_unit5].moved === 1 && unitsCountry === selectedCountry) {
      var unitsId = unitsInfo[_unit5].units_id;
      var unitSpan = document.getElementById("unit_" + unitsId);
      unitsInfo[_unit5].moved = 0;
      var unitSrc = "terrain/" + tPath + "/" + selectedCountry + unitsInfo[_unit5].units_name.replace(" ", "").toLowerCase() + ".gif";
      unitSpan.firstChild.src = unitSrc;
    }
  }
}

function changeColors() {}

var fogCanvas = document.createElement("canvas");
var fogCtx = fogCanvas.getContext("2d");
fogCanvas.id = "fog-canvas";
fogCanvas.style.zIndex = "104";
fogCanvas.style.pointerEvents = "none";

if (fogArray) {
  map.insertAdjacentElement("beforeBegin", fogCanvas);
}

drawFogCanvas(fogArray);

function drawFogCanvas(fogInfo) {
  if (!fogInfo) return;
  fogCanvas.height = map.height;
  fogCanvas.width = map.width;
  fogCtx.fillStyle = "rgba(0, 0, 0, 0.3)";
  fogCtx.fillRect(0, 0, fogCanvas.width, fogCanvas.height);

  for (var x in fogInfo) {
    for (var y in fogInfo[x]) {
      changeBuildingZIndex(x, y, 100);

      if (fogInfo[x][y] >= 1) {
        fogCtx.clearRect(x * 16, y * 16, 16, 16); //If building is in vision, put it above the fog canvas

        changeBuildingZIndex(x, y, 110);
      }
    }
  }
}

function changeBuildingZIndex(x, y, zIndex) {
  if (buildingsInfo[x]) {
    var b = buildingsInfo[x][y];

    if (b) {
      var id = b.buildings_id || b.id;
      var buildingEl = document.getElementById("building_" + id);
      buildingEl.style.zIndex = zIndex;
    }
  }
}

function resetCreatedTiles(tileClasses) {
  var movementTiles = Array.prototype.slice.call(document.querySelectorAll(tileClasses));
  movementTiles.forEach(function (t) {
    t.parentElement.removeChild(t);
  });
}