var calculator = new Vue({
  el: "#calculator",
  data: function data() {
    return {
      attacker: {
        cities: 0,
        co: {
          co_name: "andy",
          co_id: 1
        },
        country: {
          code: "os",
          name: "orangestar"
        },
        funds: 0,
        hp: 10,
        power: "N",
        hasAmmo: true,
        terrain: {
          terrain_name: "Plain",
          terrain_id: 1,
          terrain_defense: 1
        },
        towers: 0,
        unit: {
          units_name: "infantry",
          units_id: 1
        }
      },
      cos: [],
      coTheme: "",
      damage: {
        maxInfo: null,
        minInfo: null,
        maxCounterInfo: null,
        minCounterInfo: null
      },
      defender: {
        cities: 0,
        co: {
          co_name: "andy",
          co_id: 1
        },
        country: {
          code: "bm",
          name: "bluemoon"
        },
        funds: 0,
        hp: 10,
        power: "N",
        hasAmmo: true,
        terrain: {
          terrain_name: "Plain",
          terrain_id: 1,
          terrain_defense: 1
        },
        towers: 0,
        unit: {
          units_name: "infantry",
          units_id: 1
        }
      },
      dragging: false,
      error: "",
      lastMenu: {
        position: "",
        menu: ""
      },
      menuToggles: {
        attacker: {
          co: false,
          unit: false,
          terrain: false
        },
        defender: {
          co: false,
          unit: false,
          terrain: false
        }
      },
      selectorPosition: null,
      shortcutPressed: "",
      terrain: [],
      terrainPath: "terrain/aw2/",
      toggled: false,
      units: [],
      units_without_ammo: ["infantry", "recon", "lander", "apc", "t-copter", "blackboat", "blackbomb"],
      windowPosition: {
        x: 0,
        y: 0,
        offsetX: null,
        offsetY: null
      }
    };
  },
  computed: {
    //Check if HP is set to ? for Sonja's units
    attackerHpDisplay: function attackerHpDisplay() {
      return this.attacker.hp === "?" ? "qhp" : this.attacker.hp;
    },
    defenderHpDisplay: function defenderHpDisplay() {
      return this.defender.hp === "?" ? "qhp" : this.defender.hp;
    },
    defRemainingHP: function defRemainingHP() {
      var minHP = Math.ceil(this.defender.hp - this.damage.maxInfo.percent / 10);
      var maxHP = Math.ceil(this.defender.hp - this.damage.minInfo.percent / 10);

      if (this.attacker.hp === "?" || this.defender.hp === "?") {
        minHP = "?";
        maxHP = "?";
      }

      return {
        minHP: minHP < 0 ? 0 : minHP,
        maxHP: maxHP < 0 ? 0 : maxHP
      };
    }
  },
  watch: {
    //When attacker or defender change, run functions
    attacker: {
      handler: function handler() {
        this.updateDamage();
        this.storeCalcInfo("attacker", this.attacker);
      },
      deep: true
    },
    defender: {
      handler: function handler() {
        this.updateDamage();
        this.storeCalcInfo("defender", this.defender);
      },
      deep: true
    },
    "attacker.hp": function attackerHp() {
      this.checkHP("attacker");
    },
    "defender.hp": function defenderHp() {
      this.checkHP("defender");
    },
    toggled: function toggled() {
      applyCSS(calculator.$refs.calculator, {
        display: function () {
          return calculator.toggled ? "block" : "none";
        }()
      });
    }
  },
  methods: {
    getCOImage: function getCOImage(co) {
      var blankCOPath = "terrain/co-portraits/qco.png";
      if (!co || !co.co_id) return blankCOPath;
      this.cos.forEach(function (coData) {
        if (coData.co_id == co.co_id || coData.co_name == co.co_name) co = coData;
      });
      return co.co_image_path ? co.co_image_path : blankCOPath;
    },
    addUnitToCalc: function addUnitToCalc(position, id) {
      if (calculatorEl.style.display != "block") {
        calculatorEl.style.display = "block";
        calculator.storeCalcInfo("display", "block");
      } //"Info" variables are in game.php and moveplanner.php


      var unit = unitsInfo[id];
      var x = unit.units_x,
          y = unit.units_y;
      var terrain = terrainInfo[x] && terrainInfo[x][y] ? terrainInfo[x][y] : buildingsInfo[x][y];
      var playerId = unit.units_players_id ? unit.units_players_id : unit.players_id;
      var player = playersInfo[playerId];
      var newProps = {
        cities: GAME_FLAGS__KINDLE_SCOP ? player.numProperties : player.cities,
        co: {
          co_name: player.co_name.replace(" ", ""),
          co_id: player.players_co_id
        },
        country: {
          code: player.countries_code,
          name: player.countries_name.replace(" ", "")
        },
        funds: player.players_funds,
        hp: unit.units_hit_points,
        power: player.players_co_power_on,
        terrain: {
          terrain_name: terrain.terrain_name.replace(/[_ ]/g, ""),
          terrain_id: terrain.terrain_id,
          terrain_defense: terrain.terrain_defense
        },
        towers: player.towers,
        unit: {
          units_ammo: unit.units_ammo,
          units_name: unit.units_name.replace(" ", ""),
          units_id: genericUnits[unit.units_name].units_id
        }
      };

      for (var prop in newProps) {
        calculator[position][prop] = newProps[prop];
      }

      this.setAmmoButton(position, calculator[position]["unit"]);
    },
    unitUsesAmmo: function unitUsesAmmo(unitName) {
      return !this.units_without_ammo.includes(unitName.toLowerCase());
    },
    setAmmoButton: function setAmmoButton(position, unit) {
      // Set the hasAmmo property based on if the selected unit is supposed to have ammo, but doesn't
      var unitName = unit["units_name"];
      var currentAmmo = unit["units_ammo"];

      if (currentAmmo === 0 && this.unitUsesAmmo(unitName)) {
        calculator[position]["hasAmmo"] = false;
      } else {
        calculator[position]["hasAmmo"] = true;
      }
    },
    changePower: function changePower(position, type, e) {
      this.$refs[position + "-cop"].className = "";
      this.$refs[position + "-scop"].className = "";

      if (this[position].power === type) {
        e.currentTarget.className = "";
      } else {
        e.currentTarget.className = "green-border";
      }

      this[position].power === type ? this[position].power = "N" : this[position].power = type;
    },
    //Change HP if input <0 or >10
    checkHP: function checkHP(position) {
      var hp = this[position].hp;

      if (/\D/g.test(hp)) {
        this[position].hp = hp.replace(/\D/g, function (match) {
          return match.includes("?") ? "?" : "";
        });
      }

      if (hp < 0) {
        this[position].hp = 0;
      }

      if (hp > 10) {
        this[position].hp = 10;
      }
    },
    dragWindow: function dragWindow(e) {
      if (this.dragging) {
        var offsetX = this.windowPosition.offsetX;
        var offsetY = this.windowPosition.offsetY;
        applyCSS(this.$refs.calculator, {
          left: e.pageX - offsetX + "px",
          top: e.pageY - offsetY + "px"
        });
        this.windowPosition.x = e.pageX - offsetX;
        this.windowPosition.y = e.pageY - offsetY;
      }
    },
    fetchUnitId: function fetchUnitId(e) {
      var x,
          y = 0;

      if (e.type === "touchend") {
        x = e.changedTouches[0].clientX;
        y = e.changedTouches[0].clientY;
      } else {
        // triggered from click event
        x = e.clientX;
        y = e.clientY;
      }

      var span = document.elementsFromPoint(x, y).filter(function (x) {
        return x.className.includes("game-unit");
      })[0];

      if (!span) {
        span = e.target.closest("span");
      }

      var clickedUnit;

      if (span) {
        //Refreshless game page uses attributes
        clickedUnit = span.getAttribute("data-unit-id") || /open_/.test(span.id) && span.id || /unit_/.test(span.id) && span.id;
      } //Check for refreshless game page


      if (clickedUnit && calculator.selectorPosition) {
        var unitId = clickedUnit.match(/\d+/)[0];
        gamemap.removeEventListener("click", this.fetchUnitId);
        gamemap.removeEventListener("touchend", this.fetchUnitId);
        this.addUnitToCalc(this.selectorPosition, parseInt(unitId));
        this.selectState("", "white", "auto");
        this.selectorPosition = null;
        this.shortcutPressed = "";
      }
    },
    //Load calculator's default data
    populateCalc: function populateCalc() {
      axios.get("api/calculator/calculator_new_load.php").then(function (res) {
        for (var prop in res.data) {
          calculator[prop] = res.data[prop];
        }

        calculator.storeCalcInfo("coTheme", res.data.coTheme); // console.log(calculator.attacker)
      })["catch"](function (err) {
        if (err) {
          calculator.error = "Something wrong happened, try again";
          console.log("ERROR: " + err);
        }
      });
    },
    //Change the "Select" buttons colors
    selectState: function selectState(background, color, cursor) {
      var selectBtn = this.$refs["select-" + this.selectorPosition];
      applyCSS(gamemap, {
        cursor: cursor
      });
      applyCSS(selectBtn, {
        background: background,
        color: color
      });
    },
    //Upon click on the "Select" buttons
    selectUnit: function selectUnit(position) {
      if (!this.selectorPosition) {
        this.selectorPosition = this.selectorPosition = position;
        this.selectState("#DDDDDD", "black", "grab", position);
        gamemap.addEventListener("click", this.fetchUnitId);
        gamemap.addEventListener("touchend", this.fetchUnitId);

        if (moving) {
          resetCreatedTiles("span[class$='tile'], span[class$='square'], .action-square");
          resetUnit();
        }
      } else {
        this.selectState("", "white", "auto");
        this.selectorPosition = null;
        this.shortcutPressed = "";
        gamemap.removeEventListener("click", this.fetchUnitId);
        gamemap.removeEventListener("touchend", this.fetchUnitId);
      }
    },
    startDrag: function startDrag(e) {
      if (e.target === this.$refs.grab) {
        var x = this.windowPosition.x;
        var y = this.windowPosition.y;
        applyCSS(this.$refs.grab, {
          cursor: "grabbing"
        });
        this.dragging = true;
        this.windowPosition.offsetX = e.pageX - x;
        this.windowPosition.offsetY = e.pageY - y;
      }
    },
    stopDrag: function stopDrag() {
      applyCSS(this.$refs.grab, {
        cursor: "grab"
      });
      this.dragging = false;
      this.storeCalcInfo("windowPosition", this.windowPosition);
    },
    storeCalcInfo: function storeCalcInfo(storedProp, calcInfo) {
      if (sessionStorage.calculator) {
        var calcSessionInfo = JSON.parse(sessionStorage.calculator);
        calcSessionInfo[storedProp] = calcInfo;
        sessionStorage.calculator = JSON.stringify(calcSessionInfo);
      } else {
        sessionStorage.calculator = JSON.stringify({});
      }
    },
    //Swap attacker and defender
    swapPosition: function swapPosition() {
      var tempPosition = this.defender;
      this.defender = this.attacker;
      this.attacker = tempPosition;
    },
    toggleMenu: function toggleMenu(position, menu) {
      if (this.lastMenu.menu) {
        this.menuToggles[this.lastMenu.position][this.lastMenu.menu] = false;
      }

      if (position != this.lastMenu.position || menu != this.lastMenu.menu) {
        this.menuToggles[position][menu] = true;
        this.lastMenu.position = position;
        this.lastMenu.menu = menu;
      } else {
        this.lastMenu.position = "";
        this.lastMenu.menu = "";
      }
    },
    setUnit: function setUnit(position, unit) {
      this.setAmmoButton(position, unit);
      this[position]["unit"] = unit;
    },
    //Different function for touch event for easier handling
    touchMove: function touchMove(e) {
      //prevent mouse click
      e.preventDefault();
      var _this$windowPosition = this.windowPosition,
          x = _this$windowPosition.x,
          y = _this$windowPosition.y,
          offsetX = _this$windowPosition.offsetX,
          offsetY = _this$windowPosition.offsetY;
      var touch = e.touches[0];
      applyCSS(this.$refs.calculator, {
        left: touch.pageX - offsetX + "px",
        top: touch.pageY - offsetY + "px"
      });
      this.windowPosition.x = touch.pageX - offsetX;
      this.windowPosition.y = touch.pageY - offsetY;
    },

    /*
    touchStart: function(e) {
      e.preventDefault();
      const {x, y} = this.windowPosition;
      this.windowPosition.offsetX = e.touches[0].pageX - x;
      this.windowPosition.offsetY = e.touches[0].pageY - y;
      document.getElementById("bm").textContent = "bm"
    }, */
    updateDamage: function updateDamage() {
      var _this = this;

      // console.log(JSON.stringify(this.attacker));
      axios.post("api/calculator/calculate_new.php", {
        attacker: this.attacker,
        defender: this.defender,
        gameId: gameId
      }).then(function (res) {
        _this.damage = {
          maxInfo: res.data.maxInfo,
          minInfo: res.data.minInfo,
          maxCounterInfo: res.data.maxCounterInfo,
          minCounterInfo: res.data.minCounterInfo
        };
      })["catch"](function (err) {
        if (err) {
          _this.error = "Something wrong happened, try again";
        }
      });
    },
    tileImage: function tileImage(name) {
      if (typeof this.canvas === "undefined") {
        this.canvas = document.createElement("canvas");
      }

      var image = window.mapRenderer.drawTileImage(name.toLowerCase().replaceAll(" ", ""), this.canvas).toDataURL();
      return image;
    }
  },
  mounted: function mounted() {
    var _this2 = this;

    // wait for graphics to load
    window.mapRenderer.getSpriteSheet().then(function () {
      _this2.populateCalc();

      var fireOptions; //Listeners for moving the calculator around

      window.addEventListener("mouseup", _this2.stopDrag);
      window.addEventListener("mousemove", _this2.dragWindow);
      var calculatorWindow = window.getComputedStyle(_this2.$refs.calculator);
      _this2.windowPosition.x = parseInt(calculatorWindow.getPropertyValue("left"));
      _this2.windowPosition.y = parseInt(calculatorWindow.getPropertyValue("top"));
    });
  }
});
var calculatorEl = document.getElementById("calculator");
var calculatorToggle = document.querySelector(".calculator-toggle");
var calculatorClose = document.querySelector(".close-calc");
var toolsButtonBg = document.querySelector("#game_tools .borderwhite");
var toolsMenuDropdown = document.getElementById("showlinks"); //Load calculator info on page reload

if (sessionStorage.calculator) {
  var calcSessionInfo = JSON.parse(sessionStorage.calculator);

  for (var prop in calcSessionInfo) {
    calculator[prop] = calcSessionInfo[prop];
  }

  if (calcSessionInfo.windowPosition) {
    applyCSS(calculatorEl, {
      left: calcSessionInfo.windowPosition.x + "px",
      top: calcSessionInfo.windowPosition.y + "px"
    });
  }
}

calculatorToggle.addEventListener("click", function () {
  if (!sessionStorage.calculator) {
    sessionStorage.calculator = JSON.stringify({});
  }

  calculator.storeCalcInfo("toggled", !calculator.toggled);
  calculator.toggled = !calculator.toggled;

  if (toolsButtonBg && toolsMenuDropdown) {
    applyCSS(toolsButtonBg, {
      backgroundColor: "white"
    });
    applyCSS(toolsMenuDropdown, {
      display: "none"
    });
  }
});
calculatorClose.addEventListener("click", function () {
  calculator.storeCalcInfo("toggled", false);
  calculator.toggled = false;
});