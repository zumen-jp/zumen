// 設備と部品の合成入力。属性位置の根拠と旧版の推定は docs/formats/ifc.md に記録する。
use super::Case;

pub(super) const CASES: &[Case] = &[
    Case {
        name: "IfcActuator",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcActuatorType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcAirTerminal",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcAirTerminalBox",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcAirTerminalBoxType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcAirTerminalType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcAirToAirHeatRecovery",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcAirToAirHeatRecoveryType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcAlarm",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcAlarmType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcAudioVisualAppliance",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcAudioVisualApplianceType",
        predefined: [255, 10, 10],
    },
    Case {
        name: "IfcBoiler",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcBoilerType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcBuildingElementPart",
        predefined: [0, 9, 9],
    },
    Case {
        name: "IfcBuildingElementPartType",
        predefined: [255, 10, 10],
    },
    Case {
        name: "IfcBurner",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcBurnerType",
        predefined: [255, 10, 10],
    },
    Case {
        name: "IfcCableCarrierFitting",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcCableCarrierFittingType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcCableCarrierSegment",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcCableCarrierSegmentType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcCableFitting",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcCableFittingType",
        predefined: [255, 10, 10],
    },
    Case {
        name: "IfcCableSegment",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcCableSegmentType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcChiller",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcChillerType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcCoil",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcCoilType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcCommunicationsAppliance",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcCommunicationsApplianceType",
        predefined: [255, 10, 10],
    },
    Case {
        name: "IfcCompressor",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcCompressorType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcCondenser",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcCondenserType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcController",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcControllerType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcCooledBeam",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcCooledBeamType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcCoolingTower",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcCoolingTowerType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcDamper",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcDamperType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcDiscreteAccessory",
        predefined: [0, 9, 9],
    },
    Case {
        name: "IfcDiscreteAccessoryType",
        predefined: [0, 10, 10],
    },
    Case {
        name: "IfcDistributionChamberElement",
        predefined: [0, 9, 9],
    },
    Case {
        name: "IfcDistributionChamberElementType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcDistributionControlElement",
        predefined: [0, 0, 0],
    },
    Case {
        name: "IfcDistributionControlElementType",
        predefined: [0, 0, 0],
    },
    Case {
        name: "IfcDistributionElement",
        predefined: [0, 0, 0],
    },
    Case {
        name: "IfcDistributionElementType",
        predefined: [0, 0, 0],
    },
    Case {
        name: "IfcDistributionFlowElement",
        predefined: [0, 0, 0],
    },
    Case {
        name: "IfcDistributionFlowElementType",
        predefined: [0, 0, 0],
    },
    Case {
        name: "IfcDuctFitting",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcDuctFittingType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcDuctSegment",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcDuctSegmentType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcDuctSilencer",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcDuctSilencerType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcElectricAppliance",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcElectricApplianceType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcElectricDistributionBoard",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcElectricDistributionBoardType",
        predefined: [255, 10, 10],
    },
    Case {
        name: "IfcElectricDistributionPoint",
        predefined: [9, 255, 255],
    },
    Case {
        name: "IfcElectricFlowStorageDevice",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcElectricFlowStorageDeviceType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcElectricGenerator",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcElectricGeneratorType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcElectricHeaterType",
        predefined: [10, 255, 255],
    },
    Case {
        name: "IfcElectricMotor",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcElectricMotorType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcElectricTimeControl",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcElectricTimeControlType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcEnergyConversionDevice",
        predefined: [0, 0, 0],
    },
    Case {
        name: "IfcEnergyConversionDeviceType",
        predefined: [0, 0, 0],
    },
    Case {
        name: "IfcEngine",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcEngineType",
        predefined: [255, 10, 10],
    },
    Case {
        name: "IfcEvaporativeCooler",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcEvaporativeCoolerType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcEvaporator",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcEvaporatorType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcFan",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcFanType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcFilter",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcFilterType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcFireSuppressionTerminal",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcFireSuppressionTerminalType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcFlowController",
        predefined: [0, 0, 0],
    },
    Case {
        name: "IfcFlowControllerType",
        predefined: [0, 0, 0],
    },
    Case {
        name: "IfcFlowFitting",
        predefined: [0, 0, 0],
    },
    Case {
        name: "IfcFlowFittingType",
        predefined: [0, 0, 0],
    },
    Case {
        name: "IfcFlowInstrument",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcFlowInstrumentType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcFlowMeter",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcFlowMeterType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcFlowMovingDevice",
        predefined: [0, 0, 0],
    },
    Case {
        name: "IfcFlowMovingDeviceType",
        predefined: [0, 0, 0],
    },
    Case {
        name: "IfcFlowSegment",
        predefined: [0, 0, 0],
    },
    Case {
        name: "IfcFlowSegmentType",
        predefined: [0, 0, 0],
    },
    Case {
        name: "IfcFlowStorageDevice",
        predefined: [0, 0, 0],
    },
    Case {
        name: "IfcFlowStorageDeviceType",
        predefined: [0, 0, 0],
    },
    Case {
        name: "IfcFlowTerminal",
        predefined: [0, 0, 0],
    },
    Case {
        name: "IfcFlowTerminalType",
        predefined: [0, 0, 0],
    },
    Case {
        name: "IfcFlowTreatmentDevice",
        predefined: [0, 0, 0],
    },
    Case {
        name: "IfcFlowTreatmentDeviceType",
        predefined: [0, 0, 0],
    },
    Case {
        name: "IfcGasTerminalType",
        predefined: [10, 255, 255],
    },
    Case {
        name: "IfcHeatExchanger",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcHeatExchangerType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcHumidifier",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcHumidifierType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcInterceptor",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcInterceptorType",
        predefined: [255, 10, 10],
    },
    Case {
        name: "IfcJunctionBox",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcJunctionBoxType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcLamp",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcLampType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcLightFixture",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcLightFixtureType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcMedicalDevice",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcMedicalDeviceType",
        predefined: [255, 10, 10],
    },
    Case {
        name: "IfcMotorConnection",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcMotorConnectionType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcOutlet",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcOutletType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcPipeFitting",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcPipeFittingType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcPipeSegment",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcPipeSegmentType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcProtectiveDevice",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcProtectiveDeviceTrippingUnit",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcProtectiveDeviceTrippingUnitType",
        predefined: [255, 10, 10],
    },
    Case {
        name: "IfcProtectiveDeviceType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcPump",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcPumpType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcSanitaryTerminal",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcSanitaryTerminalType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcSensor",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcSensorType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcSolarDevice",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcSolarDeviceType",
        predefined: [255, 10, 10],
    },
    Case {
        name: "IfcSpaceHeater",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcSpaceHeaterType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcStackTerminal",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcStackTerminalType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcSwitchingDevice",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcSwitchingDeviceType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcTank",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcTankType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcTransformer",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcTransformerType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcTransportElement",
        predefined: [9, 9, 9],
    },
    Case {
        name: "IfcTransportElementType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcTubeBundle",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcTubeBundleType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcUnitaryControlElement",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcUnitaryControlElementType",
        predefined: [255, 10, 10],
    },
    Case {
        name: "IfcUnitaryEquipment",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcUnitaryEquipmentType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcValve",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcValveType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcVibrationIsolator",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcVibrationIsolatorType",
        predefined: [10, 10, 10],
    },
    Case {
        name: "IfcWasteTerminal",
        predefined: [255, 9, 9],
    },
    Case {
        name: "IfcWasteTerminalType",
        predefined: [10, 10, 10],
    },
];
